import React from 'react';
import { useSelector } from 'react-redux';
import Link from '../Link';
import { sortItems } from '../../util/arrays';

const PinListItem = ({ item }) => {
  if (!item) return null;

  return (
    <Link
      to={`/playlist/${item.uri}`}
      className="sidebar__menu__item sidebar__menu__item--submenu"
      activeClassName="sidebar__menu__item--active"
    >
      {item.name}
    </Link>
  );
};

const PinList = () => {
  let items = useSelector((state) => state.pusher.pinned || []);
  if (items.length <= 0) return null;
  items = sortItems(items, 'name');

  return (
    <div>
      {
        items.map((item) => <PinListItem item={item} key={item.uri} />)
      }
    </div>
  );
};

export default PinList;
