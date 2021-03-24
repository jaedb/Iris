import React, { useState } from 'react';
import handleViewport from 'react-in-viewport';
import { isCached } from '../util/storage';

const Parallax = ({
  blur,
  fixedHeight,
  image: url,
  animate = true,
  inViewport,
  forwardedRef,
}) => {
  const [isLoaded, setIsLoaded] = useState(false);
  const [currentUrl, setCurrentUrl] = useState();

  const loadImage = (urlToLoad) => {
    setCurrentUrl(urlToLoad);
    if (urlToLoad && urlToLoad !== '') {
      setIsLoaded(isCached(urlToLoad));

      const imageObject = new Image();
      imageObject.src = urlToLoad;

      imageObject.onload = () => {
        setIsLoaded(true);
      };

      // No Image, so reset it
    } else {
      setIsLoaded(false);
    }
  };

  if (url !== currentUrl) {
    loadImage(url);
  }

  let className = 'parallax preserve-3d';
  className += ` parallax--${fixedHeight ? 'fixed' : 'flexible'}-height`;
  if (blur && inViewport) className += ' parallax--blur';
  if (isLoaded) className += ' parallax--loaded';
  if (animate) className += ' parallax--animate';

  const style = isLoaded && currentUrl ? { backgroundImage: `url("${currentUrl}")` } : {};

  return (
    <div className={className} ref={forwardedRef}>
      {inViewport && (
        <div className="parallax__layer preserve-3d">
          <div className="parallax__image" style={style} />
          <div className="parallax__overlay" />
        </div>
      )}
    </div>
  );
};

export default handleViewport(Parallax);
