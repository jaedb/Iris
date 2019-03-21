import React from 'react';
import { Provider } from 'react-redux';
import { BrowserRouter, Route, IndexRoute } from "react-router-dom";

// Testing-specific
import renderer from 'react-test-renderer';
import { shallow, mount, render } from 'enzyme';
const state = require('../state');

// Test subjects
import { Album } from '../../src/js/views/Album';
import * as helpers from '../../src/js/helpers';
import * as uiActions from '../../src/js/services/ui/actions';
import * as coreActions from '../../src/js/services/core/actions';

describe('<Album />', () => {

	var album = state.core.albums['jest:album:one'];

	it('should render accurately', () => {		
		const dom = shallow(
			<Album 
				album={album}
				uiActions={uiActions}
				coreActions={coreActions}
			/>
		);

		expect(dom.find('.album-view').length).toBe(1);

		expect(dom.find('h1').text()).toEqual('One');
		expect(dom.find('.wiki__text p').text()).toEqual('Wiki text');
	});
});