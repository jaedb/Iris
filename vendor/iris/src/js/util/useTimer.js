import { useEffect, useRef } from 'react';

const useInterval = (callback, delay, repeat) => {
  const savedCallback = useRef();

  useEffect(() => {
    savedCallback.current = callback;
  }, [callback]);

  useEffect(() => {
    function tick() {
      savedCallback.current();
    }

    const action = (repeat ? setInterval(tick, delay) : setTimeout(tick, delay));
    return () => (repeat ? clearInterval(action) : clearTimeout(action));
  }, [delay]);
};

export default useInterval;
