# Example web client for webrtcsink-stats-server

This web client will display live statistics as received through a
websocket connected to a `webrtcsink-stats-server`.

Usage:

``` shell
npm install
npm run dev
```

Then navigate to `http://localhost:3000/`. Once consumers are connected
to the webrtc-sink-stats-server, they should be listed on the page, clicking
on any consumer will show a modal with plots for some of the most interesting
statistics.

The stat server can also be specified through the `remote-url` search parameter,
for example you can access a distant stat server with
`http://localhost:3000?remote-uri=my-remoye.com:72522`.
