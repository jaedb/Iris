import { useRef, useEffect } from 'react';

const useTraceUpdate = (props) => {
  const prev = useRef(props);
  useEffect(() => {
    const changedProps = Object.entries(props).reduce((ps, [k, v]) => {
      if (prev.current[k] !== v) {
        ps[k] = [prev.current[k], v];
      }
      return ps;
    }, {});
    if (Object.keys(changedProps).length > 0) {
      console.debug('Props changed:', changedProps);
    }
    prev.current = props;
  });
};

export {
  useTraceUpdate,
};

export default {
  useTraceUpdate,
};
