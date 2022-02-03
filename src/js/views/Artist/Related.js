import React from 'react';
import { Grid } from '../../components/Grid';

export default ({ artist }) => {
  return (
    <div className="body related-artists">
      <section className="grid-wrapper no-top-padding">
        <Grid items={artist.related_artists} />
      </section>
    </div>
  );
};
