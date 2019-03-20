
import React from 'react';;
import ReactDOM from 'react-dom';;
import { Provider } from 'react-redux';
import { createStore } from 'redux';
import { BrowserRouter, Route, IndexRoute } from "react-router-dom";

// Testing-specific
import renderer from 'react-test-renderer';
import { shallow, mount, render } from 'enzyme';

// Test subjects
import store from '../src/js/store';
import * as coreActions from '../src/js/services/core/actions';
import * as uiActions from '../src/js/services/ui/actions';
import * as pusherActions from '../src/js/services/pusher/actions';
import * as mopidyActions from '../src/js/services/mopidy/actions';
import * as spotifyActions from '../src/js/services/spotify/actions';
import * as lastfmActions from '../src/js/services/lastfm/actions';
import * as geniusActions from '../src/js/services/genius/actions';
import * as snapcastActions from '../src/js/services/snapcast/actions';
import { App } from '../src/js/App';

describe('<App />', () => {

	it('should render', () => {
		const dom = shallow(
			<App
				history={[]}
				location={{}}
				uiActions={uiActions}
				coreActions={coreActions}
				pusherActions={pusherActions}
				mopidyActions={mopidyActions}
				spotifyActions={spotifyActions}
				lastfmActions={lastfmActions}
				geniusActions={geniusActions}
				snapcastActions={snapcastActions}
			/>
		);


	});
});

