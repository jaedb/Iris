
import store from '../src/js/store';

const state = store.getState();

/**
 * Artists
 **/
state.core.artists['jest:artist:alpha'] = {
	uri: 'jest:artist:alpha',
	name: 'Alpha',
	followers: 123,
	albums_uris: [
		'jest:album:one'
	],
	albums_total: 1,
	biography: "Alpha biography"
};

state.core.artists['jest:artist:beta'] = {
	uri: 'jest:artist:beta',
	name: 'Beta',
	followers: 100
};

state.core.artists['jest:artist:charlie'] = {
	uri: 'jest:artist:charlie',
	name: 'Charlie',
	followers: 999
};

state.core.artists['jest:artist:delta'] = {
	uri: 'jest:artist:delta',
	name: 'Delta',
	followers: 987
};

/**
 * Albums
 **/
state.core.albums['jest:album:one'] = {
	uri: 'jest:album:one',
	name: 'One',
	artists_uris: [
		'jest:artist:alpha'
	],
	tracks_uris: [
		'jest:track:one',
		'jest:track:two'
	],
	wiki: "Wiki text"
};

/**
 * Playlists
 **/
state.core.playlists['jest:playlist:one'] = {
	uri: 'jest:playlist:one',
	name: 'One',
	tracks_uris: [
		'jest:track:one',
		'jest:track:three'
	],
	tracks_total: 2
};

/**
 * Tracks
 **/
state.core.tracks['jest:track:one'] = {
	uri: 'jest:track:one',
	name: 'One',
	artists_uris: [
		'jest:artist:charlie'
	]
};

state.core.tracks['jest:track:two'] = {
	uri: 'jest:track:two',
	name: 'Two',
	artists_uris: [
		'jest:artist:alpha',
		'jest:artist:beta'
	]
};

state.core.tracks['jest:track:three'] = {
	uri: 'jest:track:three',
	name: 'Three',
	artists_uris: [
		'jest:artist:charlie',
		'jest:artist:delta'
	]
};

module.exports = state;
