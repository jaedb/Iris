import { store } from '../src/js/store';

const state = store.getState();

state.core.items['local:artist:md5:4f6e4f979e2c40c5e6ad1735804c29bc'] = {
  type: "artist",
  uri: "local:artist:md5:4f6e4f979e2c40c5e6ad1735804c29bc",
  name: "Above & Beyond",
  provider: "mopidy",
  images: [
    {
      formatted: true,
      small: "https://i.scdn.co/image/c8fde6d9f274eb40e5404dd1e806b597ca09c29d",
      medium: "https://i.scdn.co/image/4dc7080ef509c36203a131a0eab8dd5e4800d7c2",
      large: "https://i.scdn.co/image/4dc7080ef509c36203a131a0eab8dd5e4800d7c2",
      huge: "https://i.scdn.co/image/4dc7080ef509c36203a131a0eab8dd5e4800d7c2"
    }
  ],
  albums_uris: [
    "local:album:md5:1186a9e31d19405f1c6ca2ca4ec4714a",
    "local:album:md5:66fbea3593ba96a15a9d4188bebab50b"
  ],
  tracks: [
    {
      type: "track",
      uri: "local:track:Above%20%26%20Beyond/Sirens%20of%20the%20Sea%20Remixed%20%5B2009%5D/Above%20%26%20Beyond%20-%2001%20If%20I%20Could%20Fly%20%28Jaytech%20remix%29.mp3",
      name: "If I Could Fly (Jaytech remix)",
      last_modified: 1528571787982,
      artists: [
        {
          uri: "local:artist:md5:171fb07b0aca1f116a998d72582f2caf",
          name: "Above & Beyond presents OceanLab"
        }
      ],
      album: {
        uri: "local:album:md5:1186a9e31d19405f1c6ca2ca4ec4714a",
        name: "Sirens of the Sea Remixed"
      },
      added_at: 1528571787982,
      duration: 433737,
      track_number: 1,
      disc_number: 1,
      release_date: "2009",
      provider: "local",
      disc_track: "001.001"
    },
    {
      type: "track",
      uri: "local:track:Above%20%26%20Beyond/Sirens%20of%20the%20Sea%20Remixed%20%5B2009%5D/Above%20%26%20Beyond%20-%2002%20Come%20Home%20%28Michael%20Cassette%20remix%29.mp3",
      name: "Come Home (Michael Cassette remix)",
      last_modified: 1528571788421,
      artists: [
        {
          uri: "local:artist:md5:171fb07b0aca1f116a998d72582f2caf",
          name: "Above & Beyond presents OceanLab"
        }
      ],
      album: {
        uri: "local:album:md5:1186a9e31d19405f1c6ca2ca4ec4714a",
        name: "Sirens of the Sea Remixed"
      },
      added_at: 1528571788421,
      duration: 472241,
      track_number: 2,
      disc_number: 1,
      release_date: "2009",
      provider: "local",
      disc_track: "001.002"
    },
  ]
};

state.core.items['jest:artist:beta'] = {
	uri: 'jest:artist:beta',
	name: 'Beta',
	followers: 100
};

state.core.items['jest:artist:charlie'] = {
	uri: 'jest:artist:charlie',
	name: 'Charlie',
	followers: 999
};

state.core.items['jest:artist:delta'] = {
	uri: 'jest:artist:delta',
	name: 'Delta',
	followers: 987
};

/**
 * Albums
 **/
state.core.items['local:album:md5:66fbea3593ba96a15a9d4188bebab50b'] = {
	artists: [
		{
			uri: "local:artist:md5:4f6e4f979e2c40c5e6ad1735804c29bc",
			name: "Above & Beyond"
		}
	],
	tracks: [
		{
			type: "track",
			uri: "local:track:Above%20%26%20Beyond/Sirens%20of%20the%20Sea%20%5B2008%5D/Above%20%26%20Beyond%20-%2001%20Just%20Listen.mp3",
			name: "Just Listen",
			last_modified: 1528571779393,
			artists: [
				{
					uri: "local:artist:md5:171fb07b0aca1f116a998d72582f2caf",
					name: "Above & Beyond presents OceanLab"
				}
			],
			album: {
				uri: "local:album:md5:66fbea3593ba96a15a9d4188bebab50b",
				name: "Sirens of the Sea"
			},
			added_at: 1528571779393,
			duration: 230086,
			track_number: 1,
			disc_number: 1,
			release_date: "2008",
			provider: "local",
			disc_track: "001.001"
		},
		{
			type: "track",
			uri: "local:track:Above%20%26%20Beyond/Sirens%20of%20the%20Sea%20%5B2008%5D/Above%20%26%20Beyond%20-%2002%20Sirens%20of%20the%20Sea.mp3",
			name: "Sirens of the Sea",
			last_modified: 1528571780104,
			artists: [
				{
					uri: "local:artist:md5:171fb07b0aca1f116a998d72582f2caf",
					name: "Above & Beyond presents OceanLab"
				}
			],
			album: {
				uri: "local:album:md5:66fbea3593ba96a15a9d4188bebab50b",
				name: "Sirens of the Sea"
			},
			added_at: 1528571780104,
			duration: 356884,
			track_number: 2,
			disc_number: 1,
			release_date: "2008",
			provider: "local",
			disc_track: "001.002"
		},
	],
	last_modified: 1528571779393,
	type: "album",
	uri: "local:album:md5:66fbea3593ba96a15a9d4188bebab50b",
	name: "Sirens of the Sea",
	release_date: "2008",
	provider: "local",
	images: {
		formatted: true,
		small: "/local/17338e740316f18dbb5e3331ac6be6c1-500x500.jpeg",
		medium: "/local/17338e740316f18dbb5e3331ac6be6c1-500x500.jpeg",
		large: "/local/17338e740316f18dbb5e3331ac6be6c1-500x500.jpeg",
		huge: "/local/17338e740316f18dbb5e3331ac6be6c1-500x500.jpeg"
	}
};

/**
 * Playlists
 **/
state.core.items['m3u:Local%20tester.m3u8'] = {
  type: "playlist",
  uri: "m3u:Local%20tester.m3u8",
  name: "Local tester",
  last_modified: 1616053598509,
  can_edit: true,
  tracks_total: 13,
  added_at: 1616053598509,
  provider: "mopidy",
  images: {
    formatted: true,
    small: null,
    medium: null,
    large: null,
    huge: null
  },
  tracks: [
    {
      type: "track",
      uri: "spotify:track:57YNeCxXGWY4a2rQJlw5du",
      name: "Heat Of The Moment",
      artists: [
        {
          uri: "spotify:artist:1O6ddbpGxRTse2V0YEkhNH",
          name: "15grams"
        }
      ],
      album: {
        uri: "spotify:album:7stB1TDEe4BEvBM2HSXXBA",
        name: "Heat Of The Moment"
      },
      duration: 189000,
      track_number: 1,
      disc_number: 0,
      release_date: "2020",
      provider: "spotify",
      disc_track: "000.001",
      sort_id: 0
    },
    {
      type: "track",
      uri: "spotify:track:5DHSlpd0MX9DfnoAQqolfa",
      name: "Julia (Deep Diving)",
      artists: [
        {
          uri: "spotify:artist:4oLeXFyACqeem2VImYeBFe",
          name: "Fred again.."
        }
      ],
      album: {
        uri: "spotify:album:2HJCdduuXbN70tTNQyuFQn",
        name: "Julia (Deep Diving)"
      },
      duration: 275000,
      track_number: 1,
      disc_number: 0,
      release_date: "2020",
      provider: "spotify",
      disc_track: "000.001",
      sort_id: 4
    },
  ]
};

/**
 * Tracks
 **/
state.core.items['jest:track:one'] = {
	uri: 'jest:track:one',
	name: 'One',
	artists_uris: [
		'jest:artist:charlie'
	]
};

state.core.items['jest:track:two'] = {
	uri: 'jest:track:two',
	name: 'Two',
	artists_uris: [
		'jest:artist:alpha',
		'jest:artist:beta'
	]
};

state.core.items['jest:track:three'] = {
	uri: 'jest:track:three',
	name: 'Three',
	artists_uris: [
		'jest:artist:charlie',
		'jest:artist:delta'
	]
};

module.exports = state;
