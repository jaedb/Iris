Spotmop
=======

Mopidy web-based frontend that utilizes Spotify to create an interactive, user-friendly and collaborative music interface. Built and maintained by James Barnsley.

![badge](https://img.shields.io/pypi/v/mopidy-spotmop.svg?style=flat)
![badge](https://img.shields.io/pypi/dm/mopidy-spotmop.svg)
[![badge](https://img.shields.io/badge/donate-paypal-blue.svg)](https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=james%40barnsley%2enz&lc=NZ&item_name=James%20Barnsley&currency_code=NZD&bn=PP%2dDonationsBF%3abtn_donate_LG%2egif%3aNonHosted)


Requirements
--------

* Mopidy
* Mopidy-Spotify
* Spotify Premium account
* Mopidy-Local-Sqlite (recommended, not required)

Installation
--------

1. Install using pip: `sudo pip install Mopidy-Spotmop`
2. Restart Mopidy server
3. Navigate to Mopidy interface (ie http://localhost:6680/spotmop)

Features
--------

* Full web-based interface controls for Mopidy
* Uses Spotify API to deliver high-quality audio and music information
* Improved support for local libraries using SQLite extension
* Browse and manage your playlists, along with top tracks, new releases and genre browser
* Spotmop can be run completely independently of your Mopidy machine (ie on a remote server), just set your URL in the settings tab
* Push notifications between users (requires port 6681, but this can be customised to suit your environment)

Screenshots
-----------

![Overview](https://raw.githubusercontent.com/jaedb/spotmop/master/Screenshots/overview.jpg)

![Play queue](https://raw.githubusercontent.com/jaedb/spotmop/master/Screenshots/desktop-queue.jpg)

![Featured playlists](https://raw.githubusercontent.com/jaedb/spotmop/master/Screenshots/desktop-featured.jpg)

![Artist](https://raw.githubusercontent.com/jaedb/spotmop/master/Screenshots/desktop-artist.jpg)

![Single playlist](https://raw.githubusercontent.com/jaedb/spotmop/master/Screenshots/desktop-playlist.jpg)

![Dragging tracks](https://raw.githubusercontent.com/jaedb/spotmop/master/Screenshots/desktop-dragging.jpg)


To-do
-----

* Increase stability of Mopidy server (perhaps limitation of Rpi?)
* Improve websockets integration to attach users to tracks and changes within tracklist

Support
-------

* [Changelog](https://github.com/jaedb/spotmop/releases)
* [Issues and requests](https://github.com/jaedb/spotmop/issues)

