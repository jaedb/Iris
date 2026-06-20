import gi
import sys


gi.require_version('GLib', '2.0')
from gi.repository import GLib
gi.require_version('Gtk', '4.0')
from gi.repository import Gtk
gi.require_version('Gst', '1.0')
from gi.repository import Gst

def on_activate(app):
    pipeline = Gst.Pipeline.new()

    overlay = Gst.ElementFactory.make('clockoverlay', None)
    overlay.set_property('font-desc', 'Monospace 42')

    gtksink = Gst.ElementFactory.make('gtk4paintablesink', None)
    paintable = gtksink.get_property('paintable')

    if paintable.props.gl_context:
        print('Using GL')
        src = Gst.ElementFactory.make('gltestsrc', None)

        sink = Gst.ElementFactory.make('glsinkbin', None)
        sink.set_property('sink', gtksink)
    else:
        print('Not using GL')
        src = Gst.ElementFactory.make('videotestsrc', None)

        sink = Gst.Bin.new()
        convert = Gst.ElementFactory.make('videoconvert', None)

        sink.add(convert)
        sink.add(gtksink)
        convert.link(gtksink)

        sink.add_pad(Gst.GhostPad.new('sink', convert.get_static_pad('sink')))

    pipeline.add(src)
    pipeline.add(overlay)
    pipeline.add(sink)

    src.link_filtered(overlay, Gst.Caps.from_string('video/x-raw(ANY), width=640, height=480'))
    overlay.link(sink)

    window = Gtk.ApplicationWindow(application=app)
    window.set_default_size(640, 480)

    vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)

    picture = Gtk.Picture.new()
    picture.set_paintable(paintable)

    vbox.append(picture)

    label = Gtk.Label.new('Position: 00:00:00')
    vbox.append(label)

    window.set_child(vbox)
    window.present()

    app.add_window(window)

    def on_timeout():
        (res, position) = pipeline.query_position(Gst.Format.TIME)
        if not res or position == -1:
            label.set_text('Position: 00:00:00')
        else:
            seconds = int(position / 1000000000)
            minutes = int(seconds / 60)
            hours = int(minutes / 60)
            seconds = seconds % 60
            minutes = minutes % 60
            label.set_text(f'Position: {hours:02}:{minutes:02}:{seconds:02}')

        return True

    GLib.timeout_add(500, on_timeout)

    pipeline.set_state(Gst.State.PLAYING)

    def on_bus_msg(_, msg):
        match msg.type:
            case Gst.MessageType.EOS:
                app.quit()
            case Gst.MessageType.ERROR:
                (err, _) = msg.parse_error()
                print(f'Error from {msg.src.path_string()}: {err}')
                app.quit()

        return True

    bus = pipeline.get_bus()
    bus.add_watch(GLib.PRIORITY_DEFAULT, on_bus_msg)

    def on_close(_):
        window.close()
        pipeline.set_state(Gst.State.NULL)
    app.connect('shutdown', on_close)

if __name__ == '__main__':
    Gst.init(None)
    app = Gtk.Application()
    app.connect('activate', on_activate)
    res = app.run(None)
    sys.exit(res)
