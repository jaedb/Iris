import React, { useState, useEffect, useRef, useCallback } from 'react';
import { useDrop, useDrag } from 'react-dnd';
import Icon from '../Icon';
import Link from '../Link';
import { sortItems, indexToArray } from '../../util/arrays';

const DRAGGABLE_TYPE = 'COMMAND';

const Command = ({
  index,
  onExecute,
  onDrag,
  onDragEnd,
  command: {
    id,
    url,
    name,
    icon,
    colour,
  } = {},
}) => {
  const ref = useRef(null);

  const [{ isDragging }, dragRef] = useDrag({
    type: DRAGGABLE_TYPE,
    item: { index },
    collect: (monitor) => ({
      isDragging: monitor.isDragging(),
    }),
    end: onDragEnd,
  });

  const [_dropProps, dropRef] = useDrop({
    accept: DRAGGABLE_TYPE,
    hover: (item, monitor) => {
      const dragIndex = item.index;
      const hoverIndex = index;
      const hoverBoundingRect = ref.current?.getBoundingClientRect()
      const hoverMiddleY = (hoverBoundingRect.bottom - hoverBoundingRect.top) / 2;
      const hoverActualY = monitor.getClientOffset().y - hoverBoundingRect.top;

      // if dragging down, continue only when hover is smaller than middle Y
      if (dragIndex < hoverIndex && hoverActualY < hoverMiddleY) return;
      // if dragging up, continue only when hover is bigger than middle Y
      if (dragIndex > hoverIndex && hoverActualY > hoverMiddleY) return;

      onDrag(dragIndex, hoverIndex);
      item.index = hoverIndex;
    },
  })

  let className = 'list__item commands-setup__item';
  if (isDragging) className += ' list__item--dragging';

  return (
    <div className={className} ref={dropRef(ref)}>
      <div className="commands-setup__item__details">
        <div ref={dragRef}>
          <Icon
            className="commands-setup__item__drag-handle"
            name="drag_indicator"
          />
        </div>
        <div className="commands-setup__item__command-item commands__item commands__item--small">
          <Icon className="commands__item__icon" name={icon} />
          <span className={`${colour}-background commands__item__background`} />
        </div>
        <div className="commands-setup__item__url commands__item__url">
          {name || <span className="grey-text">{url}</span>}
        </div>
      </div>
      <div className="commands-setup__item__actions">
        <a className="commands-setup__item__run-button action" onClick={() => onExecute(id, true)}>
          <Icon name="play_arrow" />
        </a>
        <Link className="commands-setup__item__edit-button action" to={`/modal/edit-command/${id}`}>
          <Icon name="edit" />
        </Link>
      </div>
    </div>
  );
}

const Commands = ({
  commands,
  runCommand,
  onChange,
}) => {
  const [list, setList] = useState([]);

  useEffect(() => {
    setList(sortItems(indexToArray(commands), 'sort_order'));
  }, [commands]);

  useEffect(() => {
    setList(sortItems(indexToArray(commands), 'sort_order'));
  }, []);

  const onDrag = useCallback((dragIndex, hoverIndex) => {
    setList((prev) => {
      const next = [...prev];
      const dragItem = next[dragIndex];
      const hoverItem = next[hoverIndex];
      next[dragIndex] = hoverItem;
      next[hoverIndex] = dragItem;
      return next;
    });
  }, [list]);

  const onDragEnd = () => {
    if (!list || !list.length) return;
    const nextCommands = {};
    list.forEach((item, index) => {
      nextCommands[item.id] = {
        ...item,
        sort_order: index,
      };
    });
    onChange(nextCommands);
  };

  return (
    <div className="commands-setup__item__drag-handle list commands-setup">
      {list.map((command, index) => (
        <Command
          key={`command_${command.id}`}
          index={index}
          command={command}
          onExecute={runCommand}
          onDrag={onDrag}
          onDragEnd={onDragEnd}
        />
      ))}
    </div>
  );
};

export default Commands;
