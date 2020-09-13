import { store } from '../src/js/store';

const state = store.getState();

/**
 * Artists
 **/
state.core.items['jest:artist:alpha'] = {
	uri: 'jest:artist:alpha',
	name: 'Alpha',
	followers: 123,
	albums_uris: [
		'jest:album:one'
	],
	albums_total: 1,
	biography: "Alpha biography"
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
state.core.items['jest:album:one'] = {
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
state.core.items['jest:playlist:one'] = {
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
