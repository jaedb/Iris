import {
	arrays,
	storage,
	format,
	helpers,
} from '../../src/js/util';
import { isLoading } from '../../src/js/util/helpers';

const state = require('../state');

describe('isLoading', () => {
	let load_queue = {
		'spotify:playlist:123': 'Some load queue value',
		'stuff:and_things': 'stuff:and_things',
	}

	it('should return false when not in load_queue', () => {
		expect(isLoading(load_queue, ['not_there'])).toBe(false);
	});

	it('should return true when in load queue', () => {
		expect(isLoading(load_queue, ['stuff:and_things'])).toBe(true);
	});

	it('should return true when in load queue (regex)', () => {
		expect(isLoading(load_queue, ['(.*)playlist(.*)'])).toBe(true);
	});

	it('should return false when regex is invalid', () => {
		expect(isLoading(load_queue, ['(*.)playlist(*.)'])).toBe(false);
	});
});

// TODO: DELETE THIS, WE USE LOCALFORAGE NOW
describe('localStorage', () => {

	it('should handle keys that are not in storage', () => {
		expect(storage.get('invalid_key')).toEqual({});
		expect(storage.get('invalid_key', 'default_value')).toEqual('default_value');
	});

	it('should store data', () => {

		// Initially empty
		expect(storage.get('test_key')).toEqual({});

		// Set it
		storage.set('test_key', 'test_value');

		// Test storage
		expect(storage.get('test_key')).toEqual('test_value');
	});
});

describe('isCached', () => {

	it('should return false when not cached', () => {
		expect(storage.isCached('https://picsum.photos/200')).toBe(false);
	});

	it('should return true when cached', () => {
	    var image = new Image();
	    image.src = 'https://picsum.photos/200';
	    image.onload = function(){
			expect(storage.isCached('https://picsum.photos/200')).toBe(true);
	    }
	});
});

describe('formatImages', () => {
	
	it('should ignore already-formatted objects', () => {
		var images = [
			{
				formatted: true,
				small: 'ignored-image.jpg'
			}
		];
		expect(format.formatImages(images).small).toBe('ignored-image.jpg');
	});
	
	it('should handle Mopidy object', () => {
		var images = [
			{
				__model__: 'Image',
				width: 600,
				url: 'test-image.jpg'
			}
		];
		expect(format.formatImages(images).small).toBe('test-image.jpg');
	});
	
	it('should handle Mopidy string', () => {
		var images = ['test-image.jpg'];
		expect(format.formatImages(images).small).toBe('test-image.jpg');
	});
	
	it('should handle Spotify image', () => {
		var images = [
			{
				width: 600,
				url: 'test-image.jpg'
			}
		];
		expect(format.formatImages(images).small).toBe('test-image.jpg');
	});
	
	it('should handle LastFM image', () => {
		var images = [
			{
				size: 'small',
				'#text': 'test-image.jpg'
			}
		];
		expect(format.formatImages(images).small).toBe('test-image.jpg');
	});
	
	it('should handle Genius image', () => {
		var images = {
			small: {
				url: 'test-image.jpg'
			}
		};
		expect(format.formatImages(images).small).toBe('test-image.jpg');
	});
	
	it('should up-fill sizes', () => {
		var images = [
			{
				width: 50,
				url: 'small.jpg'
			}
		];
		expect(format.formatImages(images).medium).toBe('small.jpg');
		expect(format.formatImages(images).large).toBe('small.jpg');
		expect(format.formatImages(images).huge).toBe('small.jpg');
	});
	
	it('should down-fill sizes', () => {
		var images = [
			{
				width: 1900,
				url: 'huge.jpg'
			}
		];
		expect(format.formatImages(images).small).toBe('huge.jpg');
		expect(format.formatImages(images).medium).toBe('huge.jpg');
		expect(format.formatImages(images).large).toBe('huge.jpg');
	});
});


/**
 * TODO: Formatters
 **/





describe('uriSource', () => {	
	it('should digest uri into a string', () => {
		expect(typeof(helpers.uriType('spotify:album:123'))).toBe('string');
		expect(helpers.uriSource('spotify:album:123')).toBe('spotify');
	});
});

describe('uriType', () => {	
	it('should digest uri into a string', () => {
		expect(typeof(helpers.uriType('spotify:album:123'))).toBe('string');
		expect(helpers.uriType('spotify:album:123')).toBe('album');
	});
});

describe('sourceIcon', () => {	
	it('should digest uri into a string', () => {
		expect(typeof(helpers.sourceIcon('spotify:album:123'))).toBe('string');
		expect(helpers.sourceIcon('spotify:album:123')).toBe('spotify');
	});
});

describe('buildLink', () => {

	it('should build uri into link as a string', () => {
		var link = helpers.buildLink('spotify:album:123');
		expect(typeof(link)).toBe('string');
		expect(link).toBe('/album/spotify%3Aalbum%3A123');
	});

	it('should handle special characters', () => {
		var link = helpers.buildLink('spotify:album:http://test.com/123!@#$%^&[];<>/?" .mp3');
		expect(typeof(link)).toBe('string');
		expect(link).toBe('/album/spotify%3Aalbum%3Ahttp%3A%2F%2Ftest.com%2F123!%40%23%24%25%5E%26%5B%5D%3B%3C%3E%2F%3F%22%20.mp3');
	});
});

describe('arrayOf', () => {

	it('should return a one-dimensional array', () => {
		var items = [
			{
				uri: '123',
				name: '123'
			},
			{
				uri: '456',
				name: '456'
			}
		];
		var uris = arrays.arrayOf('uri', items);
		expect(Array.isArray(uris)).toBe(true);
		expect(uris.length).toBe(2);

		for (var uri of uris){
			expect(typeof(uri)).toBe('string');
		}
	});

	it('should remove null and undefined items', () => {
		var items = [
			{
				uri: '123',
				name: '123'
			},
			{
				uri: null,
				name: '456'
			},
			{
				name: '789'
			}
		];
		var uris = arrays.arrayOf('uri', items);
		expect(uris.length).toBe(1);
	});
});