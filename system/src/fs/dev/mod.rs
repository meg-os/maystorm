pub mod full;
pub mod null;
pub mod random;
// pub mod stdio;
pub mod zero;

use crate::assert_call_once;

pub(super) fn install_drivers() {
    assert_call_once!();

    null::Null::init();
    zero::Zero::init();
    full::Full::init();
    // random::Random::init();
    // stdio::StdIo::init();
}
