import React from 'react';
import { BrowserRouter } from "react-router-dom";

// Testing-specific
import { shallow, mount, render } from 'enzyme';
const state = require('../state');

// Test subjects
import GridItem from '../../src/js/components/GridItem';

describe('<GridItem />', () => {

	it('should handle album', () => {
		var album = state.core.albums['jest:album:one'];
		var dom = shallow(<GridItem item={album} />);
		expect(dom.find('.grid__item__name').text()).toEqual('One');
		expect(dom.find('.grid__item__secondary__content').length).toBe(1);
	});

	it('should handle artist', () => {
		var artist = state.core.artists['jest:artist:alpha'];
		var dom = shallow(<GridItem item={artist} />);
		expect(dom.find('.grid__item__name').text()).toEqual('Alpha');
		expect(dom.find('.grid__item__secondary__content').childAt(0).render().text()).toEqual('123 followers');
		expect(dom.find('.grid__item__secondary__content').childAt(1).render().text()).toEqual('1 albums');
	});

	it('should handle playlist', () => {
		var playlist = state.core.playlists['jest:playlist:one'];
		var dom = shallow(<GridItem item={playlist} />);
		expect(dom.find('.grid__item__name').text()).toEqual('One');
		expect(dom.find('.grid__item__secondary__content').render().text()).toEqual('2 tracks');
	});
});