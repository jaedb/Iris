export default (props) => {
  window.addEventListener('resize', handleWindowResize, false);

  const handleWindowResize = (e) => {
    const width = Math.max(document.documentElement.clientWidth, window.innerWidth || 0);

    if (width <= 800) {
      if (!props.slim_mode) {
        props.uiActions.setSlimMode(true);
      }
    } else if (props.slim_mode) {
      props.uiActions.setSlimMode(false);
    }
  };

  return null;
};
