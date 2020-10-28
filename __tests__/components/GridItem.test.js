import React from 'react';
import { shallow } from 'enzyme';
import { GridItem } from '../../src/js/components/GridItem';

describe('<GridItem />', () => {

  it('should handle album', () => {
    const item = {
      uri: 'spotify:album:alpha',
      name: 'One',
    };
    const dom = shallow(<GridItem item={item} />);
    expect(dom.find('.grid__item__name').text()).toEqual('One');
    expect(dom.find('.grid__item__secondary__content').length).toBe(1);
  });

  it('should handle artist', () => {
    const item = {
      uri: 'spotify:artist:alpha',
      name: 'Alpha',
      followers: 123,
      albums_uris: ['spotify:album:beta'],
    };
    const dom = shallow(<GridItem item={item} />);
    expect(dom.find('.grid__item__name').text()).toEqual('Alpha');
    expect(dom.find('.grid__item__secondary__content').childAt(0).render().text()).toEqual('123 followers');
    expect(dom.find('.grid__item__secondary__content').childAt(1).render().text()).toEqual('1 albums');
  });

  it('should handle playlist', () => {
    const item = {
      uri: 'spotify:playlist:alpha',
      name: 'One',
      tracks: [
        { uri: 'spotify:track:123' },
        { uri: 'spotify:track:456' },
      ],
    };
    const dom = shallow(<GridItem item={item} />);
    expect(dom.find('.grid__item__name').text()).toEqual('One');
    expect(dom.find('.grid__item__secondary__content').render().text()).toEqual('2 tracks');
  });
});