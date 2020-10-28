import React from 'react';
import { shallow } from 'enzyme';

import { Album } from '../../src/js/views/Album';
import * as uiActions from '../../src/js/services/ui/actions';
import * as coreActions from '../../src/js/services/core/actions';
const state = require('../state');

describe('<Album />', () => {
  const album = {
    uri: 'jest:album:one',
    name: 'One',
    artists_uris: [
      'jest:artist:alpha',
    ],
    tracks_uris: [
      'jest:track:one',
      'jest:track:two',
    ],
    wiki: 'Wiki text',
  };

  it('should render accurately', () => {
    const dom = shallow(
			<Album
				album={album}
				uiActions={uiActions}
				coreActions={coreActions}
			/>,
			{
			  disableLifecycleMethods: true,
			},
    );

    expect(dom.find('.album-view').length).toBe(1);
    expect(dom.find('h1').text()).toEqual('One');
    expect(dom.find('.wiki__text p').text()).toEqual('Wiki text');
  });
});
