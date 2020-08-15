import React from 'react';
import { useSelector, useDispatch } from 'react-redux'
import * as coreActions from '../../services/core/actions';
import Icon from '../Icon';
import Button from '../Button';

const PinButton = ({ item }) => {
  if (!item || !item.name) return null;

  const {
    removePinned,
    addPinned,
  } = coreActions;

  const dispatch = useDispatch();
  const remove = () => dispatch(removePinned(item.uri));
  const add = () => dispatch(addPinned(item));
  const isPinned = useSelector((state) => (
    state.core.pinned ? state.core.pinned.find((pinnedItem) => pinnedItem.uri === item.uri) : false
  ));

  if (isPinned) {
    return (
      <Button
        icon
        destructiveHover
        onClick={remove}
        trackingLabel="Pin"
      >
        <Icon name="star" />
      </Button>
    );
  }
  return (
    <Button
      icon
      onClick={add}
      trackingLabel="Unpin"
    >
      <Icon name="star_border" />
    </Button>
  );
};

export default PinButton;
