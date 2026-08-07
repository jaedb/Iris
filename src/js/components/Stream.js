import { useEffect, useState } from 'react';
import { useSelector } from 'react-redux';
import { SnapStream } from './SnapStream.tsx';

const Stream = () => {
  const [stream, setStream] = useState(null);
  const {
    enabled,
    streaming_enabled,
    host,
    port,
    ssl,
  } = useSelector((state) => state?.snapcast || {});

  const start = () => {
    if (stream) {
      stream.play();
    } else {
      setStream(new SnapStream(`${ssl ? 'wss' : 'ws'}://${host}:${port}`));
    }
  }

  const stop = () => {
    if (stream) {
      stream.stop();
      setStream(null);
    }
  }

  useEffect(
    () => {
      if (enabled && streaming_enabled) {
        start();
      } else {
        stop();
      }
    }, [enabled, streaming_enabled],
  );

  return null;
}

export default Stream;
