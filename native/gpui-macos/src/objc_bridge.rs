//! Handles used at the remaining objc 0.2 AppKit and GPUI callback boundaries.
//! Foundation operations use objc2; casting an object does not transfer ownership.

pub(crate) type ObjcId = *mut objc::runtime::Object;

#[allow(non_upper_case_globals)]
pub(crate) const nil: ObjcId = std::ptr::null_mut();

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSArray, NSData, NSString};
use std::ops::{Deref, DerefMut};

// objc 0.2's callback registration requires its own Encode trait. These
// transparent wrappers retain objc2's geometry layout and authoritative encoding.
macro_rules! geometry_bridge {
    ($name:ident) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Default, PartialEq)]
        pub(crate) struct $name(pub objc2_foundation::$name);

        impl Deref for $name {
            type Target = objc2_foundation::$name;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
        impl DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
        impl From<objc2_foundation::$name> for $name {
            fn from(value: objc2_foundation::$name) -> Self {
                Self(value)
            }
        }
        impl From<$name> for objc2_foundation::$name {
            fn from(value: $name) -> Self {
                value.0
            }
        }
        unsafe impl objc::Encode for $name {
            fn encode() -> objc::Encoding {
                // repr(transparent) has precisely the wrapped Foundation ABI.
                unsafe {
                    objc::Encoding::from_str(
                        &<objc2_foundation::$name as objc2::Encode>::ENCODING.to_string(),
                    )
                }
            }
        }
    };
}
geometry_bridge!(NSPoint);
geometry_bridge!(NSSize);
geometry_bridge!(NSRect);

impl NSPoint {
    pub(crate) fn new(x: f64, y: f64) -> Self {
        Self(objc2_foundation::NSPoint::new(x, y))
    }
}
impl NSSize {
    pub(crate) fn new(width: f64, height: f64) -> Self {
        Self(objc2_foundation::NSSize::new(width, height))
    }
}
impl NSRect {
    pub(crate) fn new(origin: NSPoint, size: NSSize) -> Self {
        Self(objc2_foundation::NSRect::new(origin.0, size.0))
    }
}

pub(crate) unsafe fn array_count(array: ObjcId) -> usize {
    unsafe { objc2::msg_send![array.cast::<NSArray<AnyObject>>(), count] }
}

pub(crate) unsafe fn array_item(array: ObjcId, index: usize) -> ObjcId {
    let item: *mut AnyObject =
        unsafe { objc2::msg_send![array.cast::<NSArray<AnyObject>>(), objectAtIndex: index] };
    item.cast()
}

pub(crate) unsafe fn array_from_objects(objects: &[ObjcId]) -> ObjcId {
    let array: *mut AnyObject = unsafe {
        objc2::msg_send![objc2::class!(NSArray), arrayWithObjects: objects.as_ptr().cast::<*mut AnyObject>(), count: objects.len()]
    };
    array.cast()
}

pub(crate) fn data_from_bytes(bytes: &[u8]) -> ObjcId {
    Retained::autorelease_ptr(NSData::with_bytes(bytes)).cast()
}

pub(crate) unsafe fn string_utf8(string: ObjcId) -> *const std::ffi::c_char {
    unsafe { objc2::msg_send![string.cast::<NSString>(), UTF8String] }
}

pub(crate) unsafe fn autorelease(object: ObjcId) -> ObjcId {
    let object: *mut AnyObject =
        unsafe { objc2::msg_send![object.cast::<AnyObject>(), autorelease] };
    object.cast()
}

pub(crate) unsafe fn frame(object: ObjcId) -> NSRect {
    let frame: objc2_foundation::NSRect =
        unsafe { objc2::msg_send![object.cast::<AnyObject>(), frame] };
    frame.into()
}

pub(crate) unsafe fn view_bounds(object: ObjcId) -> NSRect {
    let bounds: objc2_foundation::NSRect =
        unsafe { objc2::msg_send![object.cast::<AnyObject>(), bounds] };
    bounds.into()
}

pub(crate) unsafe fn visible_frame(object: ObjcId) -> NSRect {
    let frame: objc2_foundation::NSRect =
        unsafe { objc2::msg_send![object.cast::<AnyObject>(), visibleFrame] };
    frame.into()
}

pub(crate) unsafe fn event_location(object: ObjcId) -> NSPoint {
    let point: objc2_foundation::NSPoint =
        unsafe { objc2::msg_send![object.cast::<AnyObject>(), locationInWindow] };
    point.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc::{class, msg_send, sel, sel_impl};
    use objc2::rc::autoreleasepool;
    use objc2_foundation::NSValue;

    #[test]
    fn geometry_roundtrips_across_native_and_both_runtimes() {
        autoreleasepool(|_| unsafe {
            let point = NSPoint::new(-17.5, 4096.25);
            let size = NSSize::new(1280.5, 720.25);
            let rect = NSRect::new(point, size);
            let value: ObjcId = msg_send![class!(NSValue), valueWithRect: rect];
            assert_eq!((&*value.cast::<NSValue>()).rectValue(), rect.0);
            let value = NSValue::valueWithRect(rect.0);
            let native = Retained::as_ptr(&value).cast::<objc::runtime::Object>();
            let roundtrip: NSRect = msg_send![native, rectValue];
            assert_eq!(roundtrip, rect);

            let value: ObjcId = msg_send![class!(NSValue), valueWithPoint: point];
            assert_eq!((&*value.cast::<NSValue>()).pointValue(), point.0);
            let value: ObjcId = msg_send![class!(NSValue), valueWithSize: size];
            assert_eq!((&*value.cast::<NSValue>()).sizeValue(), size.0);
        });
    }

    #[test]
    fn arrays_retain_strings_after_the_creation_pool_drains() {
        let array = autoreleasepool(|_| unsafe {
            let string = crate::ns_string("Rotor 截图 🦀");
            let array = array_from_objects(&[string]);
            Retained::retain(array.cast::<NSArray<NSString>>()).unwrap()
        });
        autoreleasepool(|_| unsafe {
            let raw = Retained::as_ptr(&array).cast_mut().cast();
            assert_eq!(array_count(raw), 1);
            let string = array_item(raw, 0);
            assert_eq!(
                std::ffi::CStr::from_ptr(string_utf8(string))
                    .to_str()
                    .unwrap(),
                "Rotor 截图 🦀"
            );
            assert_eq!(array_count(array_from_objects(&[])), 0);
        });
    }

    #[test]
    fn data_copies_bytes_and_supports_empty_buffers() {
        autoreleasepool(|_| unsafe {
            let mut bytes = vec![0, 127, 255];
            let data = data_from_bytes(&bytes);
            bytes.fill(42);
            assert_eq!((&*data.cast::<NSData>()).to_vec(), [0, 127, 255]);
            assert!(
                (&*data_from_bytes(&[]).cast::<NSData>())
                    .to_vec()
                    .is_empty()
            );
        });
    }
}
