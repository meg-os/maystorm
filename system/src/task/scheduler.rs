use super::{executor::Executor, *};
use crate::arch::cpu::*;
use crate::rt::PersonalityContext;
use crate::sync::{
    atomic::{AtomicFlags, AtomicWrapper},
    fifo::*,
    semaphore::*,
    spinlock::*,
    LockResult, Mutex, RwLock, RwLockReadGuard,
};
use crate::system::*;
use crate::ui::window::{WindowManager, WindowTimerEvent};
use crate::*;
use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::fmt;
use core::intrinsics::transmute;
use core::num::*;
use core::ops::*;
use core::ptr::addr_of;
use core::sync::atomic::*;
use core::time::Duration;
use megstd::io::{Error, ErrorKind};
use megstd::prelude::*;
use megstd::string::*;

const THRESHOLD_BUSY_THREAD: usize = 750;
const THRESHOLD_ENTER_SAVING: usize = 500;
const THRESHOLD_LEAVE_SAVING: usize = 750;
const THRESHOLD_ENTER_MAX: usize = 850;
const THRESHOLD_LEAVE_MAX: usize = 666;

static SCHEDULER_STATE: AtomicWrapper<SchedulerState> = AtomicWrapper::empty();
static mut SCHEDULER: Option<Box<Scheduler>> = None;
static mut THREAD_POOL: ThreadPool = ThreadPool::new();
static PROCESS_POOL: ProcessPool = ProcessPool::new();

/// System Scheduler
pub struct Scheduler {
    queue_realtime: ThreadQueue,
    queue_urgent: ThreadQueue,
    queue_normal: ThreadQueue,

    locals: Box<[Box<LocalScheduler>]>,

    usage: AtomicUsize,
    usage_total: AtomicUsize,
    is_frozen: AtomicBool,

    timer_events: SpinMutex<Vec<TimerEvent>>,
    next_timer: AtomicWrapper<Timer>,
}

#[repr(usize)]
#[allow(non_camel_case_types)]
#[non_exhaustive]
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum SchedulerState {
    /// The scheduler has not started yet.
    Disabled = 0,
    /// The scheduler is running.
    Normal,
    /// The scheduler is running in energy-saving mode.
    Saving,
    /// The scheduler is running at full throttle.
    FullThrottle,
}

impl Default for SchedulerState {
    #[inline]
    fn default() -> Self {
        Self::Disabled
    }
}

impl SchedulerState {
    #[inline]
    pub const fn as_raw(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn from_raw(val: usize) -> Self {
        unsafe { transmute(val) }
    }
}

impl From<SchedulerState> for usize {
    #[inline]
    fn from(val: SchedulerState) -> Self {
        val.as_raw()
    }
}

impl From<usize> for SchedulerState {
    #[inline]
    fn from(val: usize) -> Self {
        Self::from_raw(val)
    }
}

impl Scheduler {
    /// Start scheduler and sleep forever
    pub unsafe fn start(f: fn(usize) -> (), args: usize) -> ! {
        assert_call_once!();

        const SIZE_OF_SUB_QUEUE: usize = 63;
        const SIZE_OF_MAIN_QUEUE: usize = 255;

        let queue_realtime = ThreadQueue::with_capacity(SIZE_OF_SUB_QUEUE);
        let queue_urgent = ThreadQueue::with_capacity(SIZE_OF_SUB_QUEUE);
        let queue_normal = ThreadQueue::with_capacity(SIZE_OF_MAIN_QUEUE);

        ProcessPool::shared().add(ProcessContextData::new(
            ProcessId(0),
            Priority::Idle,
            "idle",
            "/",
        ));

        let num_of_active_cpus = System::current_device().num_of_logical_cpus();
        let mut locals = Vec::with_capacity(num_of_active_cpus);
        for index in 0..num_of_active_cpus {
            locals.push(LocalScheduler::new(ProcessorIndex(index)));
        }

        unsafe {
            SCHEDULER = Some(Box::new(Self {
                queue_realtime,
                queue_urgent,
                queue_normal,
                locals: locals.into_boxed_slice(),
                usage: AtomicUsize::new(0),
                usage_total: AtomicUsize::new(0),
                is_frozen: AtomicBool::new(false),
                next_timer: AtomicWrapper::default(),
                timer_events: SpinMutex::new(Vec::new()),
            }));
        }
        fence(Ordering::SeqCst);
        SCHEDULER_STATE.store(SchedulerState::FullThrottle);

        SpawnOption::with_priority(Priority::High)
            .start_process(f, args, "System")
            .unwrap();

        loop {
            unsafe {
                assert!(Hal::cpu().is_interrupt_enabled());
                Hal::cpu().wait_for_interrupt();
            }
        }
    }

    pub unsafe fn init_second() {
        assert_call_once!();

        SpawnOption::with_priority(Priority::Realtime)
            .start(Self::_statistics_thread, 0, "Scheduler Statistics")
            .unwrap();

        // for index in 0..System::current_device().num_of_logical_cpus() {
        //     let cpuid = ProcessorIndex(index);
        //     cpuid.get().map(|v| {
        //         if v.processor_type() == ProcessorCoreType::Main {
        //             SpawnOption::with_priority(Priority::High)
        //                 .strong_affinity(cpuid)
        //                 .start(Self::_dispatch, index, &format!("dispatch_#{}", index));
        //         }
        //     });
        // }
    }

    #[inline]
    #[track_caller]
    fn shared<'a>() -> &'a Self {
        unsafe { SCHEDULER.as_ref().unwrap() }
    }

    fn _dispatch(_index: usize) {
        loop {
            Timer::sleep(Duration::from_millis(1000_000));
        }
    }

    /// Returns whether or not the thread scheduler is running.
    pub fn is_enabled() -> bool {
        match Self::current_state() {
            SchedulerState::Disabled => false,
            _ => true,
        }
    }

    #[inline]
    pub const fn is_multi_processor_capable() -> bool {
        true
    }

    /// Returns the current state of the scheduler.
    #[inline]
    pub fn current_state() -> SchedulerState {
        SCHEDULER_STATE.value()
    }

    #[inline]
    fn set_current_state(val: SchedulerState) {
        SCHEDULER_STATE.store(val);
    }

    /// All threads will stop.
    pub fn freeze(force: bool) {
        if Self::is_enabled() {
            fence(Ordering::SeqCst);

            Self::shared().is_frozen.store(true, Ordering::SeqCst);

            fence(Ordering::SeqCst);

            if force {
                Hal::cpu().broadcast_reschedule();
                return;
            }
        }
    }

    /// Get the current process running on the current processor
    #[inline]
    pub fn current_pid() -> ProcessId {
        if Self::is_enabled() {
            Self::current_thread()
                .map(|thread| thread.as_ref().pid)
                .unwrap_or_default()
        } else {
            ProcessId(0)
        }
    }

    /// Get the current thread running on the current processor
    #[inline]
    pub fn current_thread() -> Option<ThreadHandle> {
        unsafe { without_interrupts!(Self::local_scheduler().map(|sch| sch.current_thread())) }
    }

    #[inline]
    #[track_caller]
    fn current_thread_data<'a>() -> &'a mut ThreadContextData {
        Self::current_thread()
            .and_then(|v| unsafe { v._unsafe_weak() })
            .unwrap()
    }

    /// Get the personality instance associated with the current thread
    #[inline]
    pub fn current_personality<'a>() -> Option<&'a mut PersonalityContext> {
        Self::current_thread_data()
            .personality
            .as_ref()
            .map(|v| unsafe { &mut *v.get() })
    }

    /// Perform the preemption
    pub unsafe fn reschedule() {
        assert!(Hal::cpu().is_interrupt_disabled());

        if !Self::is_enabled() {
            return;
        }
        let local = Self::local_scheduler().unwrap();
        let current = local.current_thread();
        current.update_statistics();
        let priority = { current.as_ref().priority };
        let shared = Self::shared();
        if shared.next_timer.value().is_expired() {
            Self::_process_timer_events();
        }
        if shared.is_frozen.load(Ordering::SeqCst) {
            LocalScheduler::switch_context(local, local.idle);
            return;
        }
        if priority == Priority::Realtime {
            return;
        }
        if Self::is_stalled_processor(local.index) {
            LocalScheduler::switch_context(local, local.idle);
        } else if let Some(next) = shared.queue_realtime.dequeue() {
            LocalScheduler::switch_context(local, next);
        } else if let Some(next) = (priority < Priority::High)
            .then(|| shared.queue_urgent.dequeue())
            .flatten()
        {
            LocalScheduler::switch_context(local, next);
        } else if let Some(next) = (priority < Priority::Normal)
            .then(|| shared.queue_normal.dequeue())
            .flatten()
        {
            LocalScheduler::switch_context(local, next);
        } else if current.as_ref().quantum.consume() {
            if let Some(next) = local.next_thread() {
                LocalScheduler::switch_context(local, next);
            }
        }
    }

    pub fn sleep_thread() {
        unsafe {
            without_interrupts!({
                let local = Self::local_scheduler().unwrap();
                let current = local.current_thread();
                current.update_statistics();
                current
                    .as_ref()
                    .sleep_counter
                    .fetch_add(1, Ordering::SeqCst);
                LocalScheduler::switch_context(local, local.next_thread().unwrap_or(local.idle));
            });
        }
    }

    fn yield_thread() {
        unsafe {
            without_interrupts!({
                let local = Self::local_scheduler().unwrap();
                local.current_thread().update_statistics();
                LocalScheduler::switch_context(local, local.next_thread().unwrap_or(local.idle));
            });
        }
    }

    /// Get the scheduler for the current processor
    #[inline]
    #[track_caller]
    unsafe fn local_scheduler() -> Option<&'static mut Box<LocalScheduler>> {
        assert!(Hal::cpu().is_interrupt_disabled());

        SCHEDULER.as_mut().and_then(|scheduler| {
            scheduler
                .locals
                .get_mut(Hal::cpu().current_processor_index().0)
        })
    }

    /// Returns whether the specified processor is stalled or not.
    fn is_stalled_processor(index: ProcessorIndex) -> bool {
        if Self::shared().is_frozen.load(Ordering::SeqCst) {
            return true;
        }
        let is_hybrid = matches!(
            System::current_device().processor_system_type(),
            ProcessorSystemType::Hybrid
        );
        let processor_type = System::cpu(index).processor_type();
        let allowed = match Self::current_state() {
            SchedulerState::Disabled => false,
            SchedulerState::Saving => {
                if is_hybrid {
                    processor_type.is_normal_processor() && processor_type.is_efficient_processor()
                } else {
                    processor_type.is_normal_processor()
                }
            }
            SchedulerState::Normal => processor_type.is_normal_processor(),
            SchedulerState::FullThrottle => true,
        };

        !allowed
    }

    /// Get the next executable thread from the thread queue
    #[must_use]
    fn _next_thread(scheduler: &LocalScheduler) -> Option<ThreadHandle> {
        let shared = Self::shared();
        let index = scheduler.index;

        if Self::is_stalled_processor(index) {
            Some(scheduler.idle)
        } else if let Some(next) = shared.queue_realtime.dequeue() {
            Some(next)
        } else if let Some(next) = shared.queue_urgent.dequeue() {
            Some(next)
        } else if let Some(next) = shared.queue_normal.dequeue() {
            Some(next)
        } else {
            None
        }
    }

    fn _enqueue(&self, handle: ThreadHandle) {
        match handle.as_ref().priority {
            Priority::Realtime => self.queue_realtime.enqueue(handle).unwrap(),
            Priority::High | Priority::Normal | Priority::Low => {
                self.queue_normal.enqueue(handle).unwrap()
            }
            _ => unreachable!(),
        }
    }

    /// Retire Thread
    fn retire(thread: ThreadHandle) {
        let handle = thread;
        let shared = Self::shared();
        let thread = handle.as_ref();
        thread.attribute.remove(ThreadAttribute::QUEUED);
        if thread.priority == Priority::Idle {
            return;
        } else if thread.attribute.contains(ThreadAttribute::ZOMBIE) {
            ThreadPool::remove(handle);
        } else if thread.is_asleep() {
            //
        } else {
            if !thread.attribute.fetch_set(ThreadAttribute::QUEUED) {
                shared._enqueue(handle);
            }
        }
    }

    /// Add thread to the queue
    fn add(thread: ThreadHandle) {
        let handle = thread;
        let shared = Self::shared();
        let thread = handle.as_ref();
        if thread.priority == Priority::Idle || thread.attribute.contains(ThreadAttribute::ZOMBIE) {
            return;
        }
        if !thread.attribute.fetch_set(ThreadAttribute::QUEUED) {
            shared._enqueue(handle);
        }
    }

    /// Schedule a timer event
    fn _schedule_timer(event: TimerEvent) {
        let shared = Self::shared();
        let mut events = shared.timer_events.lock();
        events.push(event);
        events.sort_by_key(|v| v.timer.deadline);

        if let Some(event) = events.first() {
            let _ = shared
                .next_timer
                .fetch_update(|v| (v > event.timer).then(|| event.timer));
        }
    }

    fn _process_timer_events() {
        let shared = Self::shared();
        let mut events = shared.timer_events.lock();
        while let Some(event) = events.first() {
            if event.is_alive() {
                break;
            } else {
                events.remove(0).fire();
            }
        }
        if let Some(event) = events.first() {
            shared.next_timer.store(event.timer);
        } else {
            shared.next_timer.store(Timer::FOREVER);
        }
    }

    /// Measuring Statistics
    fn _statistics_thread(_args: usize) {
        let shared = Self::shared();

        let expect = 1_000_000;
        let interval = Duration::from_micros(expect as u64);
        let mut measure = Timer::measure_deprecated();
        loop {
            Timer::sleep(interval);

            let now = Timer::measure_deprecated();
            let actual = now.0 - measure.0;
            let actual1000 = actual as usize * 1000;

            let mut n_busy_thread = 0;
            let mut usage = 0;
            for thread in ThreadPool::shared().data.lock().values() {
                let thread = thread.clone();

                let load0 = thread.load0.swap(0, Ordering::SeqCst);
                let load = usize::min(load0 as usize * expect as usize / actual1000, 1000);
                thread.load.store(load as u32, Ordering::SeqCst);
                if thread.priority != Priority::Idle {
                    usage += load;
                    if load >= THRESHOLD_BUSY_THREAD {
                        n_busy_thread += 1;
                    }
                }

                let process = thread.pid.get().unwrap();
                process.cpu_time.fetch_add(load0 as usize, Ordering::SeqCst);
                process.load0.fetch_add(load as u32, Ordering::SeqCst);
            }

            for process in ProcessPool::shared().read().unwrap().values() {
                let process = process.clone();

                let load = process.load0.swap(0, Ordering::SeqCst);
                process.load.store(load, Ordering::SeqCst);
            }

            let device = System::current_device();
            let num_physical_cpu = device.num_of_physical_cpus();
            let num_logical_cpu = device.num_of_logical_cpus();

            let usage_total = usize::min(usage, num_logical_cpu * 1000);
            let usage_per_cpu = usize::min(usage / num_logical_cpu, 1000);
            shared.usage_total.store(usage_total, Ordering::SeqCst);
            shared.usage.store(usage_per_cpu, Ordering::SeqCst);
            let num_low_cpu =
                if matches!(device.processor_system_type(), ProcessorSystemType::Hybrid) {
                    device.num_of_efficient_cpus()
                } else {
                    num_physical_cpu
                };

            if n_busy_thread >= num_physical_cpu {
                Self::set_current_state(SchedulerState::FullThrottle);
            } else if n_busy_thread >= num_low_cpu {
                Self::set_current_state(SchedulerState::Normal);
            } else {
                match Self::current_state() {
                    SchedulerState::Disabled => (),
                    SchedulerState::Saving => {
                        if usage_total > num_low_cpu * THRESHOLD_LEAVE_SAVING {
                            Self::set_current_state(SchedulerState::Normal);
                        }
                    }
                    SchedulerState::Normal => {
                        if usage_total > num_physical_cpu * 1000 - 1000 + THRESHOLD_ENTER_MAX {
                            Self::set_current_state(SchedulerState::FullThrottle);
                        } else if usage_total < num_low_cpu * THRESHOLD_ENTER_SAVING {
                            Self::set_current_state(SchedulerState::Saving);
                        }
                    }
                    SchedulerState::FullThrottle => {
                        if usage_total < num_physical_cpu * THRESHOLD_LEAVE_MAX {
                            Self::set_current_state(SchedulerState::Normal);
                        }
                    }
                }
            }

            measure = now;
        }
    }

    #[inline]
    pub fn usage_per_cpu() -> usize {
        let shared = Self::shared();
        shared.usage.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn usage_total() -> usize {
        let shared = Self::shared();
        shared.usage_total.load(Ordering::Relaxed)
    }

    #[track_caller]
    fn spawn_thread(
        start: ThreadStart,
        arg: usize,
        name: &str,
        options: SpawnOption,
    ) -> Result<ThreadHandle, Error> {
        let current_pid = Self::current_pid();
        let pid = if options.new_process {
            let child = ProcessContextData::new(
                current_pid,
                options.priority.unwrap_or_default(),
                name,
                current_pid.cwd().as_str(),
            );
            let pid = child.pid;
            ProcessPool::shared().add(child);
            pid
        } else {
            current_pid
        };
        let target_process = pid.get().unwrap();
        let priority = options.priority.unwrap_or(target_process.priority);
        target_process.n_threads.fetch_add(1, Ordering::SeqCst);
        let thread = ThreadContextData::new(
            pid,
            priority,
            options.strong_affinity,
            name,
            Some((start, arg)),
            options.personality,
        )
        .unwrap();
        Self::add(thread);
        Ok(thread)
    }

    /// Spawning asynchronous tasks
    pub fn spawn_async(task: impl Future<Output = ()> + 'static) {
        let task = Task::new(task);
        Self::spawn_task(task);
    }

    pub fn spawn_task(task: Task) {
        let thread = Self::current_thread_data();
        if thread.executor.is_none() {
            thread.executor = Some(Executor::new());
        }
        thread.executor.as_ref().unwrap().spawn(task);
    }

    /// Performing Asynchronous Tasks
    pub fn perform_tasks() -> ! {
        let thread = Self::current_thread_data();
        thread.executor.as_ref().map(|v| v.run());
        Self::exit();
    }

    pub fn exit() -> ! {
        let thread = Self::current_thread_data();
        thread.exit();
    }

    pub fn get_idle_statistics(vec: &mut Vec<u32>) {
        vec.clear();
        for thread in ThreadPool::shared().data.lock().values() {
            if thread.priority != Priority::Idle {
                break;
            }
            vec.push(thread.load.load(Ordering::Relaxed));
        }
    }

    pub fn print_statistics(sb: &mut impl fmt::Write) {
        let max_load = 1000 * System::current_device().num_of_logical_cpus() as u32;
        writeln!(sb, "PID P #TH %CPU TIME     NAME").unwrap();
        for process in ProcessPool::shared().read().unwrap().values() {
            let process = process.clone();
            if process.pid == ProcessId(0) {
                continue;
            }

            write!(
                sb,
                "{:3} {} {:3}",
                process.pid.0,
                process.priority as usize,
                process.n_threads.load(Ordering::Relaxed),
            )
            .unwrap();

            let load = u32::min(process.load.load(Ordering::Relaxed), max_load);
            let load0 = load % 10;
            let load1 = load / 10;
            if load1 >= 10 {
                write!(sb, " {:4}", load1,).unwrap();
            } else {
                write!(sb, " {:2}.{:1}", load1, load0,).unwrap();
            }

            let time = process.cpu_time.load(Ordering::Relaxed) / 10_000;
            let dsec = time % 100;
            let sec = time / 100 % 60;
            let min = time / 60_00 % 60;
            let hour = time / 3600_00;
            if hour > 0 {
                write!(sb, " {:02}:{:02}:{:02}", hour, min, sec,).unwrap();
            } else {
                write!(sb, " {:02}:{:02}.{:02}", min, sec, dsec,).unwrap();
            }

            writeln!(sb, " {}", process.name(),).unwrap();
        }
    }

    pub fn get_thread_statistics(sb: &mut impl fmt::Write) {
        writeln!(sb, " ID PID P ST %CPU TIME     NAME").unwrap();
        for thread in ThreadPool::shared().data.lock().values() {
            if thread.pid == ProcessId(0) {
                continue;
            }

            let status_char = if thread.is_asleep() {
                'S'
            } else {
                thread.attribute.to_char()
            };

            write!(
                sb,
                "{:3} {:3} {} {}{:01x}",
                thread.handle.as_usize(),
                thread.pid.0,
                thread.priority as usize,
                status_char,
                thread.attribute.bits(),
            )
            .unwrap();

            let load = thread.load.load(Ordering::Relaxed);
            let load0 = load % 10;
            let load1 = load / 10;
            if load1 >= 10 {
                write!(sb, " {:4}", load1,).unwrap();
            } else {
                write!(sb, " {:2}.{:1}", load1, load0,).unwrap();
            }

            let time = thread.cpu_time.load(Ordering::Relaxed) / 10_000;
            let dsec = time % 100;
            let sec = time / 100 % 60;
            let min = time / 60_00 % 60;
            let hour = time / 3600_00;
            if hour > 0 {
                write!(sb, " {:02}:{:02}:{:02}", hour, min, sec,).unwrap();
            } else {
                write!(sb, " {:02}:{:02}.{:02}", min, sec, dsec,).unwrap();
            }

            writeln!(sb, " {}", thread.name()).unwrap();
        }
    }
}

/// Processor Local Scheduler
#[allow(dead_code)]
struct LocalScheduler {
    index: ProcessorIndex,
    idle: ThreadHandle,
    current: AtomicUsize,
    retired: AtomicUsize,
    irql: AtomicUsize,
}

impl LocalScheduler {
    fn new(index: ProcessorIndex) -> Box<Self> {
        let mut sb = Sb255::new();
        write!(sb, "Idle_#{}", index.0).unwrap();
        let idle = ThreadContextData::new(
            ProcessId(0),
            Priority::Idle,
            Some(index),
            sb.as_str(),
            None,
            None,
        )
        .unwrap();
        Box::new(Self {
            index,
            idle,
            current: AtomicUsize::new(idle.as_usize()),
            retired: AtomicUsize::new(0),
            irql: AtomicUsize::new(0),
        })
    }

    #[inline(never)]
    unsafe fn switch_context(_self: &'static mut Self, next: ThreadHandle) {
        let old_irql = _self.raise_irql(Irql::Dispatch);
        let current = _self.current_thread();
        if current.as_ref().handle != next.as_ref().handle {
            _self.set_retired(current);
            _self.current.store(next.as_usize(), Ordering::SeqCst);
            let _self = ();

            {
                let current = current._unsafe_weak().unwrap();
                let next = next._unsafe_weak().unwrap();
                current.context.switch(&next.context);
            }

            Scheduler::local_scheduler()
                .unwrap()
                ._switch_context_after(old_irql);
        } else {
            _self.lower_irql(old_irql);
        }
    }

    #[inline]
    unsafe fn _switch_context_after(&mut self, irql: Irql) {
        let current = self.current_thread().as_ref();
        current
            .measure
            .store(Timer::measure_deprecated().0 as usize, Ordering::SeqCst);
        let retired = self.take_retired().unwrap();
        Scheduler::retire(retired);
        self.lower_irql(irql);
    }

    #[inline]
    fn take_retired(&self) -> Option<ThreadHandle> {
        self._swap_retired(None)
    }

    #[inline]
    #[track_caller]
    fn set_retired(&self, val: ThreadHandle) {
        let old = self._swap_retired(Some(val));
        assert_eq!(old, None);
    }

    #[inline]
    fn _swap_retired(&self, val: Option<ThreadHandle>) -> Option<ThreadHandle> {
        ThreadHandle::new(
            self.retired
                .swap(val.map(|v| v.as_usize()).unwrap_or(0), Ordering::SeqCst),
        )
    }

    #[inline]
    fn current_thread(&self) -> ThreadHandle {
        unsafe { ThreadHandle::new_unchecked(self.current.load(Ordering::SeqCst)) }
    }

    #[inline]
    fn current_irql(&self) -> Irql {
        unsafe { transmute(self.irql.load(Ordering::SeqCst)) }
    }

    /// Get the next executable thread from the thread queue
    #[must_use]
    fn next_thread(&self) -> Option<ThreadHandle> {
        Scheduler::_next_thread(self)
    }

    #[inline]
    #[track_caller]
    #[must_use]
    unsafe fn raise_irql(&self, new_irql: Irql) -> Irql {
        let old_irql = self.current_irql();
        if old_irql > new_irql {
            panic!("IRQL_NOT_GREATER_OR_EQUAL {:?} > {:?}", old_irql, new_irql);
        }
        self.irql.store(new_irql as usize, Ordering::SeqCst);
        old_irql
    }

    #[inline]
    #[track_caller]
    unsafe fn lower_irql(&self, new_irql: Irql) {
        let old_irql = self.current_irql();
        if old_irql < new_irql {
            panic!("IRQL_NOT_LESS_OR_EQUAL {:?} < {:?}", old_irql, new_irql);
        }
        self.irql.store(new_irql as usize, Ordering::SeqCst);
    }
}

#[no_mangle]
pub unsafe extern "C" fn setup_new_thread() {
    let lsch = Scheduler::local_scheduler().unwrap();
    let current = lsch.current_thread().as_ref();
    current
        .measure
        .store(Timer::measure_deprecated().0 as usize, Ordering::SeqCst);
    let retired = lsch.take_retired().unwrap();
    Scheduler::retire(retired);
    lsch.lower_irql(Irql::Passive);
}

/// Build an option to start a new thread or process.
pub struct SpawnOption {
    priority: Option<Priority>,
    new_process: bool,
    personality: Option<PersonalityContext>,
    strong_affinity: Option<ProcessorIndex>,
}

impl SpawnOption {
    #[inline]
    pub const fn new() -> Self {
        Self {
            priority: None,
            new_process: false,
            personality: None,
            strong_affinity: None,
        }
    }

    #[inline]
    pub const fn with_priority(priority: Priority) -> Self {
        Self {
            priority: Some(priority),
            new_process: false,
            personality: None,
            strong_affinity: None,
        }
    }

    #[inline]
    pub fn personality(mut self, personality: PersonalityContext) -> Self {
        self.personality = Some(personality);
        self
    }

    #[inline]
    pub fn strong_affinity(mut self, strong_affinity: ProcessorIndex) -> Self {
        self.strong_affinity = (System::current_device().num_of_logical_cpus() > strong_affinity.0)
            .then(|| strong_affinity);
        self
    }

    /// Start the specified function in a new thread.
    #[inline]
    pub fn start(self, start: fn(usize), arg: usize, name: &str) -> Result<ThreadHandle, Error> {
        Scheduler::spawn_thread(start, arg, name, self)
    }

    /// Start the specified function in a new process.
    #[inline]
    pub fn start_process(
        mut self,
        start: fn(usize),
        arg: usize,
        name: &str,
    ) -> Result<ProcessId, Error> {
        self.new_process = true;
        match Scheduler::spawn_thread(start, arg, name, self) {
            Ok(v) => v.get().map(|v| v.pid).ok_or(ErrorKind::OutOfMemory.into()),
            Err(err) => Err(err),
        }
    }

    /// Start the closure in a new thread.
    ///
    /// The parameters passed follow the move semantics of Rust's closure.
    #[inline]
    pub fn spawn<F, T>(self, start: F, name: &str) -> JoinHandle<T>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        FnSpawner::spawn(start, name, self)
    }
}

/// Wrapper object to spawn the closure
struct FnSpawner<F, T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    start: F,
    mutex: Arc<Mutex<Option<T>>>,
}

impl<F, T> FnSpawner<F, T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    fn spawn(start: F, name: &str, options: SpawnOption) -> JoinHandle<T> {
        let mutex = Arc::new(Mutex::new(None));
        let boxed = Box::new(Self {
            start,
            mutex: Arc::clone(&mutex),
        });
        let ptr = Box::into_raw(boxed);
        let thread =
            Scheduler::spawn_thread(Self::_start_thread, ptr as usize, name, options).unwrap();

        JoinHandle { thread, mutex }
    }

    fn _start_thread(p: usize) {
        {
            let this = unsafe { Box::from_raw(p as *mut Self) };
            let r = (this.start)();
            *this.mutex.lock().unwrap() = Some(r);
        }
        Scheduler::exit();
    }
}

pub struct JoinHandle<T> {
    thread: ThreadHandle,
    mutex: Arc<Mutex<Option<T>>>,
}

impl<T> JoinHandle<T> {
    // pub fn thread(&self) -> &Thread

    pub fn join(self) -> Result<T, ()> {
        self.thread.join();

        match Arc::try_unwrap(self.mutex) {
            Ok(v) => {
                let t = v.into_inner().unwrap();
                t.ok_or(())
            }
            Err(_) => unreachable!(),
        }
    }
}

static mut TIMER_SOURCE: Option<Box<dyn TimerSource>> = None;

pub trait TimerSource {
    /// Monotonic timer in ms.
    fn monotonic(&self) -> u64;

    /// deprecated
    fn measure(&self) -> TimeSpec;

    /// deprecated
    fn from_duration(&self, val: Duration) -> TimeSpec;

    /// deprecated
    fn into_duration(&self, val: TimeSpec) -> Duration;
}

#[derive(Debug, Copy, Clone, Default, PartialEq, PartialOrd)]
pub struct Timer {
    deadline: TimeSpec,
}

impl Timer {
    pub const JUST: Self = Self {
        deadline: TimeSpec(0),
    };

    pub const FOREVER: Self = Self {
        deadline: TimeSpec(isize::MAX),
    };

    #[inline]
    pub const fn from_isize(val: isize) -> Self {
        Self {
            deadline: TimeSpec(val),
        }
    }

    #[inline]
    pub const fn into_isize(self) -> isize {
        self.deadline.0
    }

    #[inline]
    pub fn new(duration: Duration) -> Self {
        if duration.is_zero() {
            Timer::JUST
        } else {
            let timer = Self::timer_source();
            Timer {
                deadline: timer.measure() + duration.into(),
            }
        }
    }

    #[inline]
    pub fn epsilon() -> Self {
        let timer = Self::timer_source();
        Timer {
            deadline: timer.measure() + TimeSpec::EPSILON,
        }
    }

    #[inline]
    pub const fn is_just(&self) -> bool {
        self.deadline.0 == 0
    }

    #[inline]
    pub const fn is_forever(&self) -> bool {
        self.deadline.0 == isize::MAX
    }

    #[inline]
    pub fn is_alive(&self) -> bool {
        if self.is_just() {
            false
        } else if self.is_forever() {
            true
        } else {
            let timer = Self::timer_source();
            self.deadline > timer.measure()
        }
    }

    #[inline]
    pub fn is_expired(&self) -> bool {
        !self.is_alive()
    }

    #[inline]
    pub fn repeat_until<F>(&self, mut f: F)
    where
        F: FnMut(),
    {
        while self.is_alive() {
            f()
        }
    }

    #[inline]
    pub unsafe fn set_timer(source: Box<dyn TimerSource>) {
        TIMER_SOURCE = Some(source);
    }

    fn timer_source<'a>() -> &'a Box<dyn TimerSource> {
        unsafe { TIMER_SOURCE.as_ref().unwrap() }
    }

    // #[track_caller]
    pub fn sleep(duration: Duration) {
        if Scheduler::is_enabled() {
            let timer = Timer::new(duration);
            let event = TimerEvent::one_shot(timer);
            if timer.is_alive() {
                event.schedule();
                Scheduler::sleep_thread();
            } else {
                Scheduler::yield_thread();
            }
        } else {
            panic!("Scheduler unavailable");
        }
    }

    pub async fn sleep_async(duration: Duration) {
        let timer = Timer::new(duration);
        let sem = AsyncSemaphore::with_capacity(0, 1);
        let event = TimerEvent::async_timer(timer, sem.clone());
        event.schedule();
        sem.wait().await;
    }

    #[inline]
    fn measure_deprecated() -> TimeSpec {
        Self::timer_source().measure()
    }

    #[inline]
    pub fn monotonic() -> Duration {
        Duration::from_millis(Self::timer_source().monotonic())
    }
}

impl From<usize> for Timer {
    #[inline]
    fn from(value: usize) -> Self {
        Timer::from_isize(value as isize)
    }
}

impl From<Timer> for usize {
    #[inline]
    fn from(value: Timer) -> Self {
        value.into_isize() as usize
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeSpec(pub isize);

impl TimeSpec {
    pub const EPSILON: Self = Self(1);

    #[inline]
    fn into_duration(self) -> Duration {
        Timer::timer_source().into_duration(self)
    }

    #[inline]
    fn from_duration(val: Duration) -> TimeSpec {
        Timer::timer_source().from_duration(val)
    }
}

impl Add<Self> for TimeSpec {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        TimeSpec(self.0 + rhs.0)
    }
}

impl From<TimeSpec> for Duration {
    #[inline]
    fn from(val: TimeSpec) -> Duration {
        val.into_duration()
    }
}

impl From<Duration> for TimeSpec {
    #[inline]
    fn from(val: Duration) -> TimeSpec {
        TimeSpec::from_duration(val)
    }
}

pub struct TimerEvent {
    timer: Timer,
    timer_type: TimerType,
}

enum TimerType {
    Async(Pin<Arc<AsyncSemaphore>>),
    OneShot(ThreadHandle),
    Window(Box<WindowTimerEvent>),
}

#[allow(dead_code)]
impl TimerEvent {
    #[inline]
    pub fn one_shot(timer: Timer) -> Self {
        Self {
            timer,
            timer_type: TimerType::OneShot(Scheduler::current_thread().unwrap()),
        }
    }

    #[inline]
    pub fn async_timer(timer: Timer, sem: Pin<Arc<AsyncSemaphore>>) -> Self {
        Self {
            timer,
            timer_type: TimerType::Async(sem),
        }
    }

    #[inline]
    pub fn window(payload: WindowTimerEvent, timer: Timer) -> Self {
        Self {
            timer,
            timer_type: TimerType::Window(Box::new(payload)),
        }
    }

    #[inline]
    pub fn is_alive(&self) -> bool {
        self.timer.is_alive()
    }

    #[inline]
    pub fn schedule(self) {
        Scheduler::_schedule_timer(self)
    }

    pub fn fire(self) {
        match self.timer_type {
            TimerType::OneShot(thread) => thread.wake(),
            TimerType::Async(sem) => sem.signal(),
            TimerType::Window(payload) => WindowManager::post_timer_event(*payload),
        }
    }
}

/// Thread Priority
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Ord, PartialOrd, Eq)]
pub enum Priority {
    /// This is the lowest priority at which the processor will be idle when all other threads are waiting. This will never be scheduled.
    Idle = 0,
    /// Lower than normal proirity
    Low,
    /// This is the normal priority that is scheduled in a round-robin fashion.
    /// When the allocated quanta are consumed, they are preempted.
    Normal,
    /// Higher than normal priority
    High,
    /// Currently, the highest priority and will not be preempted.
    Realtime,
}

impl Priority {
    pub fn is_useful(self) -> bool {
        match self {
            Priority::Idle => false,
            _ => true,
        }
    }
}

impl Default for Priority {
    #[inline]
    fn default() -> Self {
        Self::Normal
    }
}

pub struct Quantum {
    current: AtomicU8,
    default: u8,
}

impl Quantum {
    #[inline]
    pub const fn new(value: u8) -> Self {
        Self {
            current: AtomicU8::new(value),
            default: value,
        }
    }

    #[inline]
    pub fn reset(&self) {
        self.current.store(self.default, Ordering::Release);
    }

    #[inline]
    pub fn consume(&self) -> bool {
        loop {
            let current = self.current.load(Ordering::Relaxed);
            let (new, result) = if current > 1 {
                (current - 1, false)
            } else {
                (self.default, true)
            };
            match self.current.compare_exchange_weak(
                current,
                new,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => return result,
                Err(_) => (),
            }
        }
    }
}

impl From<Priority> for Quantum {
    fn from(priority: Priority) -> Self {
        match priority {
            Priority::High => Quantum::new(25),
            Priority::Normal => Quantum::new(10),
            Priority::Low => Quantum::new(5),
            _ => Quantum::new(1),
        }
    }
}

#[derive(Default)]
struct ProcessPool {
    data: RwLock<BTreeMap<ProcessId, Arc<ProcessContextData>>>,
}

impl ProcessPool {
    #[inline]
    const fn new() -> Self {
        Self {
            data: RwLock::new(BTreeMap::new()),
        }
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        &PROCESS_POOL
    }

    #[inline]
    #[track_caller]
    fn add(&self, process: ProcessContextData) {
        let key = process.pid;
        self.data.write().unwrap().insert(key, Arc::new(process));
    }

    #[inline]
    #[track_caller]
    fn remove(&self, handle: ProcessId) {
        self.data.write().unwrap().remove(&handle);
    }

    #[inline]
    fn read(&self) -> LockResult<RwLockReadGuard<BTreeMap<ProcessId, Arc<ProcessContextData>>>> {
        self.data.read()
    }

    #[inline]
    fn get(&self, handle: ProcessId) -> Option<Arc<ProcessContextData>> {
        self.data.read().unwrap().get(&handle).map(|v| v.clone())
    }
}

#[derive(Default)]
struct ThreadPool {
    data: SpinMutex<BTreeMap<ThreadHandle, Arc<ThreadContextData>>>,
}

impl ThreadPool {
    #[inline]
    const fn new() -> Self {
        Self {
            data: SpinMutex::new(BTreeMap::new()),
        }
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { &*addr_of!(THREAD_POOL) }
    }

    #[inline]
    fn add(thread: ThreadContextData) {
        let handle = thread.handle;
        Self::shared().data.lock().insert(handle, Arc::new(thread));
    }

    #[inline]
    fn remove(handle: ThreadHandle) {
        Self::shared().data.lock().remove(&handle);
    }

    #[inline]
    unsafe fn _unsafe_weak<'a>(&self, key: ThreadHandle) -> Option<&'a mut ThreadContextData> {
        self.data
            .lock()
            .get(&key)
            .map(|v| &mut *(Arc::as_ptr(v) as *mut _))
    }

    #[inline]
    #[must_use]
    fn get<'a>(&self, key: ThreadHandle) -> Option<Arc<ThreadContextData>> {
        self.data.lock().get(&key).map(|v| v.clone())
    }
}

#[repr(transparent)]
#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct ProcessId(usize);

impl ProcessId {
    #[inline]
    #[must_use]
    fn get(&self) -> Option<Arc<ProcessContextData>> {
        ProcessPool::shared().get(*self)
    }

    #[inline]
    pub fn join(&self) {
        self.get().map(|t| t.sem.wait());
    }

    pub fn cwd(&self) -> String {
        self.get()
            .map(|v| v.cwd.read().unwrap().clone())
            .unwrap_or("".to_owned())
    }

    #[inline]
    pub fn set_cwd(&self, path: &str) {
        self.get()
            .map(|v| *v.cwd.write().unwrap() = path.to_owned());
    }
}

impl From<ProcessId> for usize {
    #[inline]
    fn from(val: ProcessId) -> Self {
        val.0
    }
}

#[allow(dead_code)]
struct ProcessContextData {
    name: String,

    parent: ProcessId,
    pid: ProcessId,
    n_threads: AtomicUsize,
    priority: Priority,
    sem: Semaphore,

    start_time: TimeSpec,
    cpu_time: AtomicUsize,
    load0: AtomicU32,
    load: AtomicU32,

    cwd: RwLock<String>,
}

impl ProcessContextData {
    fn new(parent: ProcessId, priority: Priority, name: &str, cwd: &str) -> ProcessContextData {
        let pid = Self::next_pid();
        Self {
            name: name.to_string(),
            parent,
            pid,
            n_threads: AtomicUsize::new(0),
            priority,
            sem: Semaphore::new(0),
            start_time: Timer::monotonic().into(),
            cpu_time: AtomicUsize::new(0),
            load0: AtomicU32::new(0),
            load: AtomicU32::new(0),
            cwd: RwLock::new(cwd.to_owned()),
        }
    }

    #[inline]
    fn next_pid() -> ProcessId {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(0);
        ProcessId(NEXT_PID.fetch_add(1, Ordering::SeqCst))
    }

    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn exit(&self) {
        self.sem.signal();
        ProcessPool::shared().remove(self.pid);
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct ThreadHandle(NonZeroUsize);

impl ThreadHandle {
    #[inline]
    pub const fn new(val: usize) -> Option<Self> {
        match NonZeroUsize::new(val) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    #[inline]
    const unsafe fn new_unchecked(val: usize) -> Self {
        Self(NonZeroUsize::new_unchecked(val))
    }

    /// Acquire the next thread ID
    #[inline]
    fn next() -> ThreadHandle {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        ThreadHandle::new(NEXT_ID.fetch_add(1, Ordering::Relaxed)).unwrap()
    }

    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0.get()
    }

    #[inline]
    fn get(&self) -> Option<Arc<ThreadContextData>> {
        ThreadPool::shared().get(*self)
    }

    #[inline]
    #[track_caller]
    fn as_ref(&self) -> Arc<ThreadContextData> {
        self.get().unwrap()
    }

    #[inline]
    #[track_caller]
    unsafe fn _unsafe_weak<'a>(&self) -> Option<&'a mut ThreadContextData> {
        ThreadPool::shared()._unsafe_weak(*self)
    }

    #[inline]
    pub fn name(&self) -> Option<String> {
        self.get().map(|v| v.name())
    }

    #[inline]
    pub fn wake(&self) {
        let Some(thread) = self.get() else { return };
        thread.sleep_counter.fetch_sub(1, Ordering::SeqCst);
        Scheduler::add(*self);
    }

    #[inline]
    pub fn join(&self) {
        self.get().map(|thread| thread.sem.wait());
    }

    #[inline]
    pub fn strong_affinity(&self) -> Option<ProcessorIndex> {
        self.get().and_then(|v| v.strong_affinity)
    }

    fn update_statistics(&self) {
        let Some(thread) = self.get() else { return };

        let now = Timer::measure_deprecated().0 as usize;
        let then = thread.measure.swap(now, Ordering::SeqCst);
        let diff = now - then;
        thread.cpu_time.fetch_add(diff, Ordering::SeqCst);
        thread.load0.fetch_add(diff as u32, Ordering::SeqCst);
    }
}

type ThreadStart = fn(usize) -> ();

#[allow(dead_code)]
struct ThreadContextData {
    /// Architectural context data
    context: CpuContextData,

    stack: Option<Box<[u8]>>,

    // IDs
    pid: ProcessId,
    handle: ThreadHandle,

    // Properties
    name: String,
    sem: Semaphore,
    personality: Option<UnsafeCell<PersonalityContext>>,
    attribute: AtomicFlags<ThreadAttribute>,
    sleep_counter: AtomicIsize,
    priority: Priority,
    strong_affinity: Option<ProcessorIndex>,
    quantum: Quantum,

    // Statistics
    measure: AtomicUsize,
    cpu_time: AtomicUsize,
    load0: AtomicU32,
    load: AtomicU32,

    // Executor
    executor: Option<Executor>,
}

my_bitflags! {
    struct ThreadAttribute: usize {
        const QUEUED    = 0b0000_0000_0000_0001;
        const ZOMBIE    = 0b0000_0000_0000_1000;
    }
}

impl AtomicFlags<ThreadAttribute> {
    fn to_char(&self) -> char {
        if self.contains(ThreadAttribute::ZOMBIE) {
            'z'
        } else if self.contains(ThreadAttribute::QUEUED) {
            'R'
        } else {
            '-'
        }
    }
}

impl fmt::Display for AtomicFlags<ThreadAttribute> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_char())
    }
}

#[allow(dead_code)]
impl ThreadContextData {
    fn new(
        pid: ProcessId,
        priority: Priority,
        strong_affinity: Option<ProcessorIndex>,
        name: &str,
        start: Option<(ThreadStart, usize)>,
        personality: Option<PersonalityContext>,
    ) -> Result<ThreadHandle, ()> {
        let handle = ThreadHandle::next();

        let mut thread = Self {
            context: CpuContextData::new(),
            stack: None,
            pid,
            handle,
            sem: Semaphore::new(0),
            attribute: AtomicFlags::empty(),
            sleep_counter: AtomicIsize::new(0),
            priority,
            strong_affinity,
            quantum: Quantum::from(priority),
            measure: AtomicUsize::new(0),
            cpu_time: AtomicUsize::new(0),
            load0: AtomicU32::new(0),
            load: AtomicU32::new(0),
            executor: None,
            personality: personality.map(|v| UnsafeCell::new(v)),
            name: name.to_string(),
        };
        if let Some((start, arg)) = start {
            unsafe {
                let size_of_stack = CpuContextData::SIZE_OF_STACK;
                let mut stack = Vec::with_capacity(size_of_stack);
                stack.resize(size_of_stack, 0);
                let stack = stack.into_boxed_slice();
                thread.stack = Some(stack);
                let stack = thread.stack.as_mut().unwrap().as_mut_ptr() as *mut c_void;
                thread
                    .context
                    .init(stack.add(size_of_stack), start as usize, arg);
            }
        }
        ThreadPool::add(thread);
        Ok(handle)
    }

    fn exit(&mut self) -> ! {
        Scheduler::yield_thread();

        self.sem.signal();
        if let Some(context) = self.personality.take() {
            context.into_inner().on_exit();
        }

        let process = self.pid.get().unwrap();
        if process.n_threads.fetch_sub(1, Ordering::SeqCst) == 1 {
            process.exit();
        }

        self.attribute.insert(ThreadAttribute::ZOMBIE);
        Scheduler::yield_thread();
        unreachable!();
    }

    #[inline]
    fn is_asleep(&self) -> bool {
        self.sleep_counter.load(Ordering::Relaxed) > 0
    }

    fn name(&self) -> String {
        self.name.as_str().to_owned()
    }
}

#[repr(transparent)]
struct ThreadQueue(ConcurrentFifo<ThreadHandle>);

impl ThreadQueue {
    #[inline]
    fn with_capacity(capacity: usize) -> Self {
        Self(ConcurrentFifo::with_capacity(capacity))
    }

    #[inline]
    fn dequeue(&self) -> Option<ThreadHandle> {
        self.0.dequeue()
    }

    #[inline]
    fn enqueue(&self, data: ThreadHandle) -> Result<(), ()> {
        self.0.enqueue(data).map_err(|_| ())
    }
}

/// Interrupt Request Level
#[repr(usize)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Irql {
    Passive = 0,
    Apc,
    Dispatch,
    Device,
    IPI,
    High,
}

impl Irql {
    #[inline]
    pub fn current() -> Irql {
        unsafe {
            Scheduler::local_scheduler()
                .map(|v| v.current_irql())
                .unwrap_or(Irql::Passive)
        }
    }

    #[inline]
    #[track_caller]
    pub unsafe fn raise<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        without_interrupts!(match Scheduler::local_scheduler() {
            Some(lsch) => {
                let old_irql = lsch.raise_irql(*self);
                let r = f();
                Scheduler::local_scheduler().unwrap().lower_irql(old_irql);
                r
            }
            // TODO: can't change irql
            None => f(),
        })
    }
}
