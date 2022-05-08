import { useSelector } from 'react-redux';
import { useHistory, useLocation } from 'react-router-dom';

const ModalStateListener = () => {
  const { name } = useSelector((state) => state.ui.modal || {});
  const history = useHistory();
  const location = useLocation();

  if (name && location.pathname.indexOf(`modal/${name}`) < 0) {
    history.push(`modal/${name}`);
  }

  return null;
}

export default ModalStateListener;
