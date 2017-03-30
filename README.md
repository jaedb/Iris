Iris
=======

Iris (formerly known as Spotmop) is a Mopidy HTTP client that utilizes Spotify to create an interactive, user-friendly and collaborative music interface. Built and maintained by James Barnsley.

[![npm](https://img.shields.io/npm/v/mopidy-iris.svg?style=flat-square)]()
![badge](https://img.shields.io/pypi/v/mopidy-iris.svg?style=flat-square)
![badge](https://img.shields.io/badge/unique_monthly_users-2,600+-brightgreen.svg?style=flat-square)
[![badge](https://img.shields.io/badge/donate-paypal-blue.svg?style=flat-square)](https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=james%40barnsley%2enz&lc=NZ&item_name=James%20Barnsley&currency_code=NZD&bn=PP%2dDonationsBF%3abtn_donate_LG%2egif%3aNonHosted)

Features
--------

* Full web-based interface controls for Mopidy
* Integrated with Spotify and LastFM APIs for high-quality artwork and extra info
* Improved support for local libraries using SQLite extension
* Browse and manage your playlists, along with top tracks, new releases and genre browser
* Push notifications between clients (requires port 6681, but this can be customised to suit your environment)

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
2. Install all development requirements `npm install`
3. Run build `npm run prod` (once production build has been run once, you can use `npm run dev` for faster dev builds) 
4. Install Python module in develop mode `python setup.py install develop`
5. Restart mopidy `pkill mopidy && mopidy &`

Support
-------

* [Releases and changelog](https://github.com/jaedb/iris/releases)
* [Issues and requests](https://github.com/jaedb/Iris/wiki/Logging-an-issue)

