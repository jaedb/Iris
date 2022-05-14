import React, { useEffect } from 'react';
import { useDispatch } from 'react-redux';
import { closeModal } from '../../services/ui/actions';
import Icon from '../../components/Icon';

const Modal = ({
  extraControls,
  noclose,
  children,
  className = '',
}) => {
  const dispatch = useDispatch();

  useEffect(() => {
    $('body').addClass('modal-open');
    return () => {
      $('body').removeClass('modal-open');
    }
  }, []);

  const onClose = () => {
    dispatch(closeModal());
  }

  return (
    <div className={`modal ${className}`}>

      <div className="controls">
        {extraControls}
        {!noclose && (
          <div className="control close" onClick={onClose}>
            <Icon name="close" className="white" />
          </div>
        )}
      </div>

      <div className="content">
        {children}
      </div>
    </div>
  );
}

export default Modal;
