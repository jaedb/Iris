import React from 'react';
import { useSelector, useDispatch } from 'react-redux';
import * as pusherActions from '../../services/pusher/actions';
import Icon from '../Icon';
import Button from '../Button';

const PinButton = ({ item, unpinIcon = 'star', pinIcon = 'star_border' }) => {
  if (!item || !item.name) return null;

  const {
    addPinned,
    removePinned,
  } = pusherActions;

  const dispatch = useDispatch();
  const remove = () => dispatch(removePinned(item.uri));
  const add = () => dispatch(addPinned(item));
  const isPinned = useSelector((state) => (
    state.pusher.pinned
      ? state.pusher.pinned.find((pinnedItem) => pinnedItem.uri === item.uri)
      : false
  ));

  if (isPinned) {
    return (
      <Button
        icon
        destructiveHover
        onClick={remove}
        tracking={{ category: 'PinButton', action: 'Pin' }}
      >
        <Icon name={unpinIcon} />
      </Button>
    );
  }
  return (
    <Button
      icon
      onClick={add}
      tracking={{ category: 'PinButton', action: 'Unpin' }}
    >
      <Icon name={pinIcon} />
    </Button>
  );
};

export default PinButton;
