import React, { memo } from 'react';
import Thumbnail from './Thumbnail';
import URILink from './URILink';

export default memo(({ artists, uiActions: { showContextMenu } = {} }) => {
  if (!artists) return null;

  const onContextMenu = (e, item) => {
    if (showContextMenu) {
      e.preventDefault();
      showContextMenu({
        e,
        context: 'artist',
        uris: [item.uri],
        items: [item],
      });
    }
  };

  return (
    <div className="related-artists">
      {artists.map((artist) => {
        let { images } = artist;
        if (Array.isArray(images)) {
          images = images[0];
        }

        if (artist.uri) {
          return (
            <URILink
              type="artist"
              uri={artist.uri}
              key={artist.uri}
              className="related-artists__item related-artists__item--link"
              handleContextMenu={(e) => onContextMenu(e, artist)}
            >
              <Thumbnail className="related-artists__item__thumbnail" circle size="small" images={images} />
              <span className="related-artists__item__name">{ artist.name }</span>
            </URILink>
          );
        }
        return (
          <span key={artist.uri} className="related-artists__item">
            <Thumbnail
              className="related-artists__item__thumbnail"
              circle
              size="small"
              images={images}
            />
            <span className="related-artists__item__name">{ artist.name }</span>
          </span>
        );
      })}
    </div>
  );
});
