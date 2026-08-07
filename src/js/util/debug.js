import { useRef, useEffect } from 'react';

/**
 * Drop this in to a component

componentDidUpdate(prevProps, prevState) {
  Object.entries(this.props).forEach(([key, val]) =>
    prevProps[key] !== val && console.log(`Prop '${key}' changed`)
  );
  if (this.state) {
    Object.entries(this.state).forEach(([key, val]) =>
      prevState[key] !== val && console.log(`State '${key}' changed`)
    );
  }
}
 */

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
