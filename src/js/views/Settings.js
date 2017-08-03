
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'
import { Link } from 'react-router'

import Header from '../components/Header'
import System from '../components/Settings/System'
import Services from '../components/Settings/Services'
import Debug from '../components/Settings/Debug'

export default class Settings extends React.Component {

	renderSubViewMenu(){		
		return (
			<div className="sub-views">
				<Link className="option" activeClassName="active" to={global.baseURL+'settings/'}><h4>System</h4></Link>
				<Link className="option" activeClassName="active" to={global.baseURL+'settings/services'}><h4>Services</h4></Link>
				<Link className="option" activeClassName="active" to={global.baseURL+'settings/debug'}><h4>Debug</h4></Link>
				<Link className="option" activeClassName="active" to={global.baseURL+'settings/about'}><h4>About</h4></Link>
			</div>
		)
	}

	renderSubView(){
		switch (this.props.params.sub_view){

			case 'services':
				return (
					<div className="body related-artists">
						<Services />
					</div>
				)

			case 'debug':
				return (
					<div className="body debug">
						<Debug />
					</div>
				)

			case 'about':
				return (
					<div className="body about">
						<div className="field">
							<div>
								<em><a href="https://github.com/jaedb/Iris" target="_blank">Iris</a></em> is an open-source project by <a href="https://github.com/jaedb" target="_blank">James Barnsley</a>. It is provided free and with absolutely no warranty. If you paid someone for this software, please let me know.
									<br />
									<br />
									Google Analytics is used to help trace issues and provide valuable insight into how we can continue to make improvements.
									<br />
							</div>
							<br /><br />
							<div>
						        <a className="button" href="https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=james%40barnsley%2enz&lc=NZ&item_name=James%20Barnsley&currency_code=USD&bn=PP%2dDonationsBF%3abtn_donate_LG%2egif%3aNonHosted" target="_blank">
						        	<FontAwesome name="paypal" />&nbsp;Donate
						        </a>
						        &nbsp;&nbsp;
						        <a className="button" href="https://github.com/jaedb/Iris" target="_blank">
						        	<FontAwesome name="github" />&nbsp;GitHub
						        </a>
						        &nbsp;&nbsp;
						        <a rel="license" href="http://creativecommons.org/licenses/by-nc/4.0/" target="_blank" style={{display: 'inline-block', verticalAlign: 'middle'}}><img alt="Creative Commons License" src="https://i.creativecommons.org/l/by-nc/4.0/88x31.png" /></a>
							</div>
				        </div>
					</div>
				)

			default:
				return (
					<div className="body system">
						<System />
					</div>
				)
		}
	}

	render(){
		return (
			<div className="view settings-view">
				<Header icon="cog" title="Settings" />

				<section className="content-wrapper">

					{this.renderSubViewMenu()}
					{this.renderSubView()}

		        </section>

			</div>
		);
	}
}