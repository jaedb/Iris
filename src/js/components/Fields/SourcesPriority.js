import React, { useState, useEffect } from 'react';
import { useDispatch } from 'react-redux';
import { ReactSortable } from 'react-sortablejs';
import { set as uiSet } from '../../services/ui/actions';
import Icon from '../Icon';
import { titleCase } from '../../util/helpers';

const SourcesPriority = ({
  uri_schemes,
  uri_schemes_priority,
}) => {
  const [list, setList] = useState([]);
  const dispatch = useDispatch();

  useEffect(() => {
    processList();
  }, []);

  useEffect(() => {
    processList();
  }, [uri_schemes]);

  const processList = () => {
    let seen = [];
    let unseen = [];
    uri_schemes.forEach((uri) => {
      const index = uri_schemes_priority.indexOf(uri);
      if (index > -1) {
        seen[index] = { uri };
      } else {
        unseen.push({ uri });
      }
    });
    setList([ ...seen, ...unseen ]);
  }

  const onSort = () => {
    dispatch(uiSet({ uri_schemes_priority: list.map(({ uri }) => uri) }));
  }

  return (
    <ReactSortable
      options={{
        animation: 150,
      }}
      className="sources-priority-field"
      list={list}
      setList={setList}
      onSort={onSort}
    >
      {
        list.map(({ uri }) => {
          const name = titleCase(uri.replace(':', '').replace('+', ' '));

          return (
            <span className="source flag flag--grey" key={`uri_scheme_${uri}`}>
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
