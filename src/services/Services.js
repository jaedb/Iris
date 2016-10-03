
import ServicesContainer from 'kioc';

import MopidyService from './MopidyService';
import SpotifyService from './SpotifyService';

var Services = new ServicesContainer();
Services.use(MopidyService, true);
//Services.use(SpotifyService, true);

export default Services;