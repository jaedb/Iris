import React from 'react';
import sanitizeHtml from 'sanitize-html';
import Thumbnail from '../../components/Thumbnail';
import Icon, { SourceIcon } from '../../components/Icon';
import {
  uriSource,
  titleCase,
} from '../../util/helpers';
import { i18n, I18n } from '../../locale';
import { nice_number } from '../../components/NiceNumber';

const ArtistAbout = ({
  artist,
}) => {
  const thumbnails = artist.images && Array.isArray(artist.images) && artist.images.map(
    (image) => {
      if (!image.huge) return null;
      return (
        <div className="tile thumbnail-wrapper" key={image.huge}>
          <Thumbnail size="huge" canZoom fill images={image} />
        </div>
      );
    },
  );

  return (
    <div className="body about">
      <div className="col col--w40 tiles artist-stats">
        {thumbnails}
        <div className="tile">
          <span className="content">
            <SourceIcon uri={artist.uri} />
            <I18n
              path="artist.about.source"
              source={titleCase(uriSource(artist.uri))}
            />
          </span>
        </div>
        {artist.followers && (
          <div className="tile">
            <span className="content">
              <Icon type="fontawesome" name="users" />
              <I18n path="specs.followers" count={nice_number(artist.followers)} />
            </span>
          </div>
        )}
        {artist.popularity && (
          <div className="tile">
            <span className="content">
              <Icon type="fontawesome" name="fire" />
              <I18n path="specs.popularity" percent={artist.popularity} />
            </span>
          </div>
        )}
        {artist.listeners && (
          <div className="tile">
            <span className="content">
              <Icon type="fontawesome" name="headphones" />
              <I18n path="specs.listeners" count={nice_number(artist.listeners)} />
            </span>
          </div>
        )}
      </div>

      <div className="col col--w60 biography">
        <section>
          <br />
          {artist.biography && (
            <div className="biography-text">
              <p dangerouslySetInnerHTML={{ __html: sanitizeHtml(artist.biography) }}></p>
              <br />
              <div className="mid_grey-text">
                <I18n path="artist.about.wiki.published" date={artist.biography_publish_date} />
              </div>
              <div className="mid_grey-text">
                <I18n path="artist.about.wiki.origin" />
                <a
                  href={artist.biography_link}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  {artist.biography_link}
                </a>
              </div>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}

export default ArtistAbout;
