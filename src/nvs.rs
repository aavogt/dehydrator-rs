use std::{ptr::null_mut, ffi::CString, mem::transmute, iter::Step, error::Error, fmt::{Display, Formatter}};

use esp_idf_svc::nvs::{EspNvs, NvsCustom, NvsDefault};
use esp_idf_sys::{nvs_entry_find, nvs_type_t_NVS_TYPE_BLOB, nvs_entry_info_t, nvs_entry_info, nvs_entry_next, EspError};

#[derive(Clone,Copy,PartialOrd,PartialEq)]
pub struct Key ([u8; 16]);

impl Key {
    pub fn get_first_comp() -> Key {
        unsafe {
            let iter = null_mut();
            let partition = CString::new("measured").unwrap();
            let namespace = CString::new("comp").unwrap();

            nvs_entry_find(partition.as_ptr(),
                    namespace.as_ptr(),
                    nvs_type_t_NVS_TYPE_BLOB,
                    iter);
            let mut info : nvs_entry_info_t = Default::default();
            nvs_entry_info(*iter, &mut info as *mut _); 
            return Key(transmute(info.key));
        }
    }
    pub fn get_last_comp() -> Key {
        unsafe {
            let iter = null_mut();
            let partition = CString::new("measured").unwrap();
            let namespace = CString::new("comp").unwrap();

            nvs_entry_find(partition.as_ptr(),
                    namespace.as_ptr(),
                    nvs_type_t_NVS_TYPE_BLOB,
                    iter);
            let mut info : nvs_entry_info_t = Default::default();
            while iter != null_mut() {
                nvs_entry_info(*iter, &mut info as *mut _); 
                nvs_entry_next(iter);
            }
            return Key(transmute(info.key));
        }
    }

    pub fn to_str(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(&self.0) }
    }

    // reversed keys are ordered lexicographically
    // interpreted as u8 (c says it's an i8)
    pub fn next(&mut self) {
        for i in 0..16 {
            self.0[i] += 1;
            if self.0[i] != 0 { break }
        }
    }
    fn prev(&mut self) {
        for i in 0..16 {
            self.0[i] -= 1;
            if self.0[i] != 255 { break }
        }
    }

    fn subtract(&mut self, other : &Key) {
        let mut borrow = 0;
        for i in 0..16 {
            let mut diff = self.0[i] as i16 - other.0[i] as i16 - borrow;
            if diff < 0 {
                diff += 256;
                borrow = 1;
            } else {
                borrow = 0;
            }
            self.0[i] = diff as u8;
        }
    }

    fn to_usize(&self) -> Option<usize> {
        let mut ret = 0usize;
        for i in 0..16 {
            // break if we would have an overflow
            ret = ret.checked_mul(256usize)?.checked_add(self.0[i] as usize)?;
        }
        Some(ret)
    }

}

// enable k1 .. k2 syntax
impl Step for Key {
    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        let mut ret = start;
        for _ in 0..count {
            ret.next();
        }
        Some(ret)
    }

    // I don't use these
    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        let mut ret = start;
        for _ in 0..count {
            ret.prev();
        }
        Some(ret)
    }
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        let mut s = start.clone();
        s.subtract(end);
        s.to_usize()
    }
}


/// A reader/writer for ciborium that uses esp-idf nvs.
/// This allows ciborium::de::from_reader and ciborium::ser::to_writer
/// to call [esp_idf_svc::nvs::EspNvs::set_blob]
/// and     [esp_idf_svc::nvs::EspNvs::get_blob]
pub struct ReadWrite<'a> (pub &'a mut EspNvs<NvsCustom>, pub &'a mut Key);

impl <'a> ciborium_io::Write for ReadWrite<'a> {
    type Error = EspError;
    fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.0.set_blob(self.1.to_str(), buf)?;
        Ok(())
    }

    /// noop because set_blob calls nvs_commit
    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl <'a> ciborium_io::Read for ReadWrite<'a> {
    type Error = EspError;
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        let _buf = self.0.get_blob(self.1.to_str(), buf)?.expect("empty blob");
        Ok(())
    }
}

/// see [ReadWrite]. There are two difference. First the key is a string slice,
/// so there is no way to "get the first/last/next key". And the NvsDefault partition
/// is used which is the first partition in `partitions.csv`
pub struct ReadWriteStr<'a> (pub &'a mut EspNvs<NvsDefault>, pub &'a str);

impl <'a> ciborium_io::Write for ReadWriteStr<'a> {
    type Error = EspError;
    fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.0.set_blob(self.1, buf)?;
        Ok(())
    }

    /// noop because set_blob calls nvs_commit
    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl <'a> ciborium_io::Read for ReadWriteStr<'a> {
    type Error = anyhow::Error;
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        let _buf = self.0.get_blob(self.1, buf)?.ok_or(anyhow::Error::new(EmptyBlob))?;
        Ok(())
    }
}

/// copy pasted except for reference
impl <'a> ciborium_io::Read for &ReadWriteStr<'a> {
    type Error = anyhow::Error;
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        let _buf = self.0.get_blob(self.1, buf)?.ok_or(anyhow::Error::new(EmptyBlob))?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct EmptyBlob;
impl Display for EmptyBlob {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "empty blob")
    }
}
impl Error for EmptyBlob {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
