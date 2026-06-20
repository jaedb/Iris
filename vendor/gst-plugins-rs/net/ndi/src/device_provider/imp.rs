// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;
use gst::subclass::prelude::*;

use std::{
    sync::{atomic, Mutex, OnceLock},
    thread,
};

use std::sync::LazyLock;

use crate::ndi;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ndideviceprovider",
        gst::DebugColorFlags::empty(),
        Some("NewTek NDI Device Provider"),
    )
});

#[derive(Debug)]
pub struct DeviceProvider {
    thread: Mutex<Option<thread::JoinHandle<()>>>,
    current_devices: Mutex<Vec<super::Device>>,
    find: Mutex<Option<ndi::FindInstance>>,
    is_running: atomic::AtomicBool,
}

#[glib::object_subclass]
impl ObjectSubclass for DeviceProvider {
    const NAME: &'static str = "GstNdiDeviceProvider";
    type Type = super::DeviceProvider;
    type ParentType = gst::DeviceProvider;

    fn new() -> Self {
        Self {
            thread: Mutex::new(None),
            current_devices: Mutex::new(vec![]),
            find: Mutex::new(None),
            is_running: atomic::AtomicBool::new(false),
        }
    }
}

impl ObjectImpl for DeviceProvider {}

impl GstObjectImpl for DeviceProvider {}

impl DeviceProviderImpl for DeviceProvider {
    fn metadata() -> Option<&'static gst::subclass::DeviceProviderMetadata> {
        static METADATA: LazyLock<gst::subclass::DeviceProviderMetadata> = LazyLock::new(|| {
            gst::subclass::DeviceProviderMetadata::new("NewTek NDI Device Provider",
            "Source/Audio/Video/Network",
            "NewTek NDI Device Provider",
            "Ruben Gonzalez <rubenrua@teltek.es>, Daniel Vilar <daniel.peiteado@teltek.es>, Sebastian Dröge <sebastian@centricular.com>")
        });

        Some(&*METADATA)
    }

    fn probe(&self) -> Vec<gst::Device> {
        self.current_devices
            .lock()
            .unwrap()
            .iter()
            .map(|d| d.clone().upcast())
            .collect()
    }

    fn start(&self) -> Result<(), gst::LoggableError> {
        if let Err(err) = crate::ndi::load() {
            return Err(gst::loggable_error!(CAT, "{}", err));
        }

        let mut thread_guard = self.thread.lock().unwrap();
        if thread_guard.is_some() {
            gst::log!(CAT, imp = self, "Device provider already started");
            return Ok(());
        }

        self.is_running.store(true, atomic::Ordering::SeqCst);

        let imp_weak = self.downgrade();
        let mut first = true;
        *thread_guard = Some(thread::spawn(move || {
            {
                let Some(imp) = imp_weak.upgrade() else {
                    return;
                };

                let mut find_guard = imp.find.lock().unwrap();
                if find_guard.is_some() {
                    gst::log!(CAT, imp = imp, "Already started");
                    return;
                }

                let find = match ndi::FindInstance::builder().build() {
                    None => {
                        gst::error!(CAT, imp = imp, "Failed to create Find instance");
                        return;
                    }
                    Some(find) => find,
                };
                *find_guard = Some(find);
            }

            loop {
                let Some(imp) = imp_weak.upgrade() else {
                    return;
                };

                if !imp.is_running.load(atomic::Ordering::SeqCst) {
                    break;
                }

                imp.poll(first);
                first = false;
            }
        }));

        Ok(())
    }

    fn stop(&self) {
        if let Some(_thread) = self.thread.lock().unwrap().take() {
            self.is_running.store(false, atomic::Ordering::SeqCst);
            // Don't actually join because that might take a while
        }
    }
}

impl DeviceProvider {
    fn poll(&self, first: bool) {
        let mut find_guard = self.find.lock().unwrap();
        let find = match *find_guard {
            None => return,
            Some(ref mut find) => find,
        };

        if !find.wait_for_sources(if first { 1000 } else { 5000 }) {
            gst::trace!(CAT, imp = self, "No new sources found");
            return;
        }

        let sources = find.get_current_sources();
        let mut sources = sources.iter().map(|s| s.to_owned()).collect::<Vec<_>>();

        let mut current_devices_guard = self.current_devices.lock().unwrap();
        let mut expired_devices = vec![];
        let mut remaining_sources = vec![];

        // First check for each device we previously knew if it's still available
        for old_device in &*current_devices_guard {
            let old_device_imp = old_device.imp();
            let old_source = old_device_imp.source.get().unwrap();

            if !sources.contains(old_source) {
                gst::log!(CAT, imp = self, "Source {:?} disappeared", old_source);
                expired_devices.push(old_device.clone());
            } else {
                // Otherwise remember that we had it before already and don't have to announce it
                // again. After the loop we're going to remove these all from the sources vec.
                remaining_sources.push(old_source.to_owned());
            }
        }

        for remaining_source in remaining_sources {
            sources.retain(|s| s != &remaining_source);
        }

        // Remove all expired devices from the list of cached devices
        current_devices_guard.retain(|d| !expired_devices.contains(d));
        // And also notify the device provider of them having disappeared
        for old_device in expired_devices {
            self.obj().device_remove(&old_device);
        }

        // Now go through all new devices and announce them
        for source in sources {
            gst::log!(CAT, imp = self, "Source {:?} appeared", source);
            let device = super::Device::new(&source);
            self.obj().device_add(&device);
            current_devices_guard.push(device);
        }
    }
}

#[derive(Debug)]
pub struct Device {
    source: OnceLock<ndi::Source<'static>>,
}

#[glib::object_subclass]
impl ObjectSubclass for Device {
    const NAME: &'static str = "GstNdiDevice";
    type Type = super::Device;
    type ParentType = gst::Device;

    fn new() -> Self {
        Self {
            source: OnceLock::new(),
        }
    }
}

impl ObjectImpl for Device {}

impl GstObjectImpl for Device {}

impl DeviceImpl for Device {
    fn create_element(&self, name: Option<&str>) -> Result<gst::Element, gst::LoggableError> {
        let source_info = self.source.get().unwrap();
        let element = gst::Object::builder::<crate::ndisrc::NdiSrc>()
            .name_if_some(name)
            .property("ndi-name", source_info.ndi_name())
            .property("url-address", source_info.url_address())
            .build()
            .unwrap()
            .upcast::<gst::Element>();

        Ok(element)
    }
}

impl super::Device {
    fn new(source: &ndi::Source<'_>) -> super::Device {
        let display_name = source.ndi_name();
        let device_class = "Source/Audio/Video/Network";

        let element_class =
            glib::Class::<gst::Element>::from_type(crate::ndisrc::NdiSrc::static_type()).unwrap();
        let templ = element_class.pad_template("src").unwrap();
        let caps = templ.caps();

        // Put the url-address into the extra properties
        let extra_properties = gst::Structure::builder("properties")
            .field("ndi-name", source.ndi_name())
            .field("url-address", source.url_address())
            .build();

        let device = gst::Object::builder::<super::Device>()
            .property("caps", caps)
            .property("display-name", display_name)
            .property("device-class", device_class)
            .property("properties", extra_properties)
            .build()
            .unwrap();

        let imp = device.imp();
        imp.source.set(source.to_owned()).unwrap();

        device
    }
}
