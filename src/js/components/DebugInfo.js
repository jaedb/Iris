import React, { useState, useEffect } from 'react';
import localForage from 'localforage';
import { get as getStorage } from '../util/storage';
import { isTouchDevice } from '../util/helpers';
import { indexToArray } from '../util/arrays';
import { useSelector } from 'react-redux';

const getLocalStorageUsage = () => {
  let data = '';

  for (const key in window.localStorage) {
    if (window.localStorage.hasOwnProperty(key)) {
      data += window.localStorage[key];
    }
  }

  let used = 0;
  const total = 5000;
  if (data !== '') {
    used = ((data.length * 16) / (8 * 1024)).toFixed(2);
  }

  return {
    used,
    percent: (used / total * 100).toFixed(2),
  };
};

const LoadQueue = () => {
  const { load_queue } = useSelector((state) => state.ui);
  
  if (!load_queue) return <div className="debug-info-item mid_grey-text">Nothing loading</div>;

  const queue = indexToArray(load_queue);

  if (queue.length > 0) {
    return (
      <div className="debug-info-item">
        {queue.map((item, index) => (<div key={`${item}_${index}`}>{item}</div>))}
      </div>
    );
  }
  return <div className="debug-info-item mid_grey-text">Nothing loading</div>;
}

const DebugInfo = ({
}) => {
  const { 
    notifications = {},
    processes = {},
    slim_mode,
    test_mode,
    selected_tracks = [],
  } = useSelector((state) => state.ui);
  const { items } = useSelector((state) => state.core);
  const { enqueue_uris_batches = [] } = useSelector((state) => state.mopidy);
  const [localForageLength, setLocalForageLength] = useState(0);
  const localStorageUsage = getLocalStorageUsage();

  useEffect(() => {
    localForage.length().then(setLocalForageLength);
  }, []);

  return (
    <div className="debug-info">
      <div className="debug-info-section">
        <div className="debug-info-item">
          {`Version: ${version}`}
        </div>
        <div className="debug-info-item">
          {`Build: ${build}`}
        </div>
        <div className="debug-info-item">
          {'Dimensions: '}
          {`${document.documentElement.clientWidth} (${window.innerWidth})w `}
          {`${document.documentElement.clientHeight} (${window.innerHeight})h `}
        </div>
        <div className="debug-info-item">
          {`Pixel ratio: ${window.devicePixelRatio}`}
        </div>
      </div>

      <div className="debug-info-section">
        <h5>State</h5>
        <div className="debug-info-item">
          {`Items: ${Object.keys(items).length}`}
        </div>
        <div className="debug-info-item">
          {`Coldstore items: ${localForageLength}`}
        </div>
        <div className="debug-info-item">
          {`Notifications: ${Object.keys(notifications).length}`}
        </div>
        <div className="debug-info-item">
          {`Processes: ${Object.keys(processes).length}`}
        </div>
        <div className="debug-info-item">
          {`Enqueue batches: ${enqueue_uris_batches.length}`}
        </div>
        <div className="debug-info-item">
          {`Cached URLs: ${Object.keys(getStorage('cache')).length}`}
        </div>
      </div>

      <div className="debug-info-section">
        <h5>Config</h5>
        <div className="debug-info-item">
          {`Slim mode: ${slim_mode ? 'on' : 'off'}`}
        </div>
        <div className="debug-info-item">
          {`Test mode: ${test_mode ? 'on' : 'off'}`}
        </div>
        <div className="debug-info-item">
          {`Touch: ${isTouchDevice() ? 'on' : 'off'}`}
        </div>
        <div className="debug-info-item">
          {`LocalStorage usage: ${localStorageUsage.used}kb (~${localStorageUsage.percent}%)`}
        </div>
        <div className="debug-info-item">
          {`Selected tracks: ${selected_tracks.length}`}
          <br />
          {
            selected_tracks.map((track_key, index) => (
              <div key={`${track_key}_${index}`}>{track_key}</div>
            ))
          }
        </div>
      </div>

      <div className="debug-info-section">
        <h5>Load queue</h5>
        <LoadQueue />
      </div>

    </div>
  );
}

export default DebugInfo;
