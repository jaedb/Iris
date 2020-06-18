
import React, { memo } from 'react';
import ContextMenuTrigger from './ContextMenuTrigger';

export default memo(({
  handleContextMenuTrigger,
  options,
  title,
  uiActions,
  className,
  children,
}) => {
  const onTrigger = (e) => {
    if (handleContextMenuTrigger) return handleContextMenuTrigger(e);

    e.preventDefault();
    const data = {
      e,
      context: 'custom',
      title,
      options,
    };
    uiActions.showContextMenu(data);
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
