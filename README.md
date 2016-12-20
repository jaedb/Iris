Iris
=======

Iris (previously known as Spotmop) is a Mopidy HTTP client that utilizes Spotify to create an interactive, user-friendly and collaborative music interface. Built and maintained by James Barnsley.

![badge](https://img.shields.io/pypi/v/mopidy-iris.svg?style=flat)
![badge](https://img.shields.io/pypi/dm/mopidy-iris.svg)
[![badge](https://img.shields.io/badge/donate-paypal-blue.svg)](https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=james%40barnsley%2enz&lc=NZ&item_name=James%20Barnsley&currency_code=NZD&bn=PP%2dDonationsBF%3abtn_donate_LG%2egif%3aNonHosted)

Features
--------

* Full web-based interface controls for Mopidy
* Integrated with Spotify and LastFM APIs for high-quality artwork and extra info
* Improved support for local libraries using SQLite extension
* Browse and manage your playlists, along with top tracks, new releases and genre browser
* Spotmop can be run completely independently of your Mopidy machine (ie on a remote server), just set your URL in the settings tab
* Push notifications between users (requires port 6681, but this can be customised to suit your environment)

Screenshots
-----------

![Overview](https://raw.githubusercontent.com/jaedb/iris/master/Screenshots/overview.jpg)

![Play queue](https://raw.githubusercontent.com/jaedb/iris/master/Screenshots/desktop-queue.jpg)

![Featured playlists](https://raw.githubusercontent.com/jaedb/iris/master/Screenshots/desktop-featured.jpg)

![Artist](https://raw.githubusercontent.com/jaedb/iris/master/Screenshots/desktop-artist.jpg)

![Single playlist](https://raw.githubusercontent.com/jaedb/iris/master/Screenshots/desktop-playlist.jpg)

![Dragging tracks](https://raw.githubusercontent.com/jaedb/iris/master/Screenshots/desktop-dragging.jpg)


Requirements
--------

* Mopidy
* Mopidy-Spotify
* Spotify Premium account
* Mopidy-Local-Sqlite (recommended, not required)

Installation
--------

1. Install using pip: `sudo pip install Mopidy-Iris`
2. Restart Mopidy server
3. Navigate to Mopidy interface (ie http://localhost:6680/iris)

Development
-----

1. Clone this repository into a your Apache2 web directory (eg `/var/www/html/`)
2. Install in develop mode `python setup.py install develop`
3. Restart mopidy

Support
-------

* [Changelog](https://github.com/jaedb/iris/releases)
* [Issues and requests](https://github.com/jaedb/iris/issues)

