
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'

import FontAwesome from 'react-fontawesome'

class URISchemesList extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		if( !this.props.mopidy.uri_schemes ) return null;

		return (
			<div className="uri-schemes-list">
				{
					this.props.mopidy.uri_schemes.map( (scheme, index) => {
						scheme = scheme.replace(':','')
						return (
							<span key={index+'_'+scheme}>
								{ index > 0 ? <span>, </span> : null }
								<span className="uri-scheme">
									{ scheme }
								</span>
							</span>
						);
					})
				}
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return state;
}

export default connect(mapStateToProps)(URISchemesList)