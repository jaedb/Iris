import { useSelector, useDispatch } from 'react-redux';
import { setSlimMode } from '../services/ui/actions';

export default () => {
  const slim_mode = useSelector((state) => state.ui.slim_mode);
  const dispatch = useDispatch();

  const handleWindowResize = () => {
    const width = Math.max(document.documentElement.clientWidth, window.innerWidth || 0);

    if (width <= 800) {
      if (!slim_mode) {
        dispatch(setSlimMode(true));
      }
    } else if (slim_mode) {
      dispatch(setSlimMode(false));
    }
  };

  window.addEventListener('resize', handleWindowResize, false);

  return null;
};
