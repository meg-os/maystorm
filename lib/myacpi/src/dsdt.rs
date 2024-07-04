use super::*;

/// Differentiated System Description Table
#[repr(C, packed)]
#[allow(dead_code)]
pub struct Dsdt {
    hdr: AcpiHeader,
}

unsafe impl AcpiTable for Dsdt {
    const TABLE_ID: TableId = TableId::DSDT;
}

/// Secondary System Descriptor Table
#[repr(C, packed)]
#[allow(dead_code)]
pub struct Ssdt {
    hdr: AcpiHeader,
}

unsafe impl AcpiTable for Ssdt {
    const TABLE_ID: TableId = TableId::SSDT;
}
