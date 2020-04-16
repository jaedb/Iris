
import React, { memo } from 'react';
import Link from './Link';
import Icon from './Icon';

export default memo((props) => {
  const mapImageSizes = () => {
    // Single image
    if (props.image) {
      return props.image;

      // Multiple images
    } if (props.images) {
      let { images } = props;

      // An array of image objects (eg Artists), so just pick the first one
      if (Array.isArray(images) && images.length > 0) {
        images = images[0];
      }

      // Default to medium-sized image, but accept size property as override
      let size = 'medium';
      if (props.size) {
        size = props.size;
      }

      // Return the requested size
      if (images[size]) {
        return images[size];
      }
    }

    // No images
    return null;
  };

  const image = mapImageSizes();
  let class_name = 'thumbnail thumbnail--loaded';

  if (props.size) {
    class_name += ` thumbnail--${props.size}`;
  }
  if (props.circle) {
    class_name += ' thumbnail--circle';
  }
  if (props.className) {
    class_name += ` ${props.className}`;
  }

  let zoom_icon = null;
  if (props.canZoom && image) {
    zoom_icon = <Link className="thumbnail__zoom" to={`/image-zoom?url=${image}`}><Icon name="search" /></Link>;
  }

  const placeholderIcon = () => {
    switch (props.type) {
      case 'artist':
      case 'user':
        return 'person';
      case 'album':
        return 'album';
      case 'playlist':
        return 'queue_music';
      case 'track':
        return 'audio_track';
      default:
        return 'image';
    };
  }
  let placeholder = 'image';

  return (
    <div className={class_name}>
      {!image && (
        <Icon className="thumbnail__placeholder" name={placeholderIcon()} />
      )}
      {props.useImageTag && image ? (
        <img
          alt="Artwork thumbnail"
          className="thumbnail__image thumbnail__image--use-image-tag"
          src={image}
        />
      ) : (
        <div
          className="thumbnail__image"
          style={{ backgroundImage: `url("${image}")` }}
        />
      )}
      {props.glow && image && <div className="thumbnail__image thumbnail__image--glow" style={{ backgroundImage: `url("${image}")` }} />}
      <div className="thumbnail__actions">
        {props.canZoom && image && <Link className="thumbnail__actions__item thumbnail__actions__item--zoom" to={`/image-zoom?url=${image}`}><Icon name="search" /></Link>}
        {props.children}
      </div>
    </div>
  );
});
