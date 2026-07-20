import React, { memo } from 'react';
import { useDispatch } from 'react-redux';
import ContextMenuTrigger from './ContextMenu/ContextMenuTrigger';
import { showContextMenu } from '../services/ui/actions';

export default memo(({
  handleContextMenuTrigger,
  options,
  title,
  className,
  children,
}) => {
  const dispatch = useDispatch();
  const onTrigger = (e) => {
    if (handleContextMenuTrigger) return handleContextMenuTrigger(e);

    e.preventDefault();
    dispatch(
      showContextMenu({
        e,
        type: 'custom',
        title,
        items: options,
      }),
    );
    return true;
  };

  return (
    <header className={className}>
      <h1>
        {children}
      </h1>
      {
        (options || handleContextMenuTrigger) && (
          <div className="header__options">
            <ContextMenuTrigger onTrigger={onTrigger} />
            <div className="header__options__wrapper">
              {options || null}
            </div>
          </div>
        )
      }
    </header>
  );
});
