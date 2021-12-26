import React, { useEffect, useState } from 'react';
import { ReactSortable } from 'react-sortablejs';
import Icon from '../Icon';
import { titleCase } from '../../util/helpers';

const SourcesPriority = ({
  uri_schemes,
  uri_schemes_priority,
  uiActions: {
    set: setUi,
  },
}) => {
  const [list, setList] = useState([]);

  useEffect(() => {
    const ordered_schemes = [];
    const unordered_schemes = [];
    for (let i = 0; i < uri_schemes.length; i++) {
      const index = uri_schemes_priority.indexOf(uri_schemes[i]);
      if (index > -1) {
        ordered_schemes[index] = uri_schemes[i];
      } else {
        unordered_schemes.push(uri_schemes[i]);
      }
    }
    for (let i = 0; i < unordered_schemes.length; i++) {
      ordered_schemes.push(unordered_schemes[i]);
    }
    setList(ordered_schemes);
  }, []);

  const onChange = (nextList) => {
    console.debug(nextList);
    setList(nextList);
    setUi({ uri_schemes_priority: nextList });
  }

  return (
    <ReactSortable
      options={{
        animation: 150,
      }}
      className="sources-priority-field"
      list={list}
      setList={onChange}
    >
      {
        list.map((item) => {
          const name = titleCase(item.replace(':', '').replace('+', ' '));
          return (
            <span className="source flag flag--grey" key={item} data-id={item}>
              <Icon name="drag_indicator" />
              {name}
            </span>
          );
        })
      }
    </ReactSortable>
  );
}

export default SourcesPriority;
