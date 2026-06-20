use gtk::glib;
use std::sync::mpsc;

pub(crate) fn invoke_on_main_thread<F, T>(func: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let context = glib::MainContext::default();

    let (send, recv) = mpsc::channel();
    context.invoke(move || {
        send.send(func()).expect("Somehow we dropped the receiver");
    });
    recv.recv().expect("Somehow we dropped the sender")
}
