import React from 'react';
import { Provider } from 'react-redux';
import { BrowserRouter, Route, IndexRoute } from "react-router-dom";

// Testing-specific
import renderer from 'react-test-renderer';
import { shallow, mount, render } from 'enzyme';

// Test subjects
import { Album } from '../../src/js/views/Album';
import store from '../../src/js/store';
import * as helpers from '../../src/js/helpers';
import * as uiActions from '../../src/js/services/ui/actions';
import * as coreActions from '../../src/js/services/core/actions';


describe('<Album />', () => {

	var album = {
		uri: 'jest:album:test',
		name: 'Test album',
		tracks: {
			
		}
	}

	it('should render', () => {		
		const dom = shallow(
			<Album 
				album={album}
				uiActions={uiActions}
				coreActions={coreActions}
			/>
		);

		expect(dom.find('.album-view').length).toBe(1);
	});
});