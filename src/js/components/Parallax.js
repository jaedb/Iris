import React, { useState } from 'react';
import handleViewport from 'react-in-viewport';
import { isCached } from '../util/storage';

const Parallax = ({
  blur,
  fixedHeight,
  image: url,
  animate = true,
}) => {
  const [isLoaded, setIsLoaded] = useState(false);
  const [currentUrl, setCurrentUrl] = useState();

  const loadImage = (urlToLoad) => {
    console.debug('loadImage', { urlToLoad, currentUrl, url, isLoaded })

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
  if (blur) className += ' parallax--blur';
  if (isLoaded) className += ' parallax--loaded';
  if (animate) className += ' parallax--animate';

  const style = isLoaded && currentUrl ? { backgroundImage: `url("${currentUrl}")` } : {};

  return (
    <div className={className}>
      <div className="parallax__layer preserve-3d">
        <div className="parallax__image" style={style} />
        <div className="parallax__overlay" />
      </div>
    </div>
  );
}

export default Parallax;
