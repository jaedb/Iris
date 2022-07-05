import { useSelector } from 'react-redux';
import { useNavigate, useLocation } from 'react-router-dom';

const ModalStateListener = () => {
  const { name } = useSelector((state) => state.ui.modal || {});
  const navigate = useNavigate();
  const location = useLocation();

  if (name && location.pathname.indexOf(`modal/${name}`) < 0) {
    navigate(`modal/${name}`);
  }

  return null;
}

export default ModalStateListener;
