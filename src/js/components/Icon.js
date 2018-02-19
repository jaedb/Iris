
import React, { PropTypes } from 'react';
import { connect } from 'react-redux';
import { createStore, bindActionCreators } from 'redux';

import * as helpers from '../helpers';
import * as uiActions from '../services/ui/actions';

class Icon extends React.Component{

	constructor(props){
		super(props);
	}

	componentDidMount(){
		if (!this.props.icons[this.props.name]){
			this.props.uiActions.getIcon(this.props.name);
		}
	}

	componentWillReceiveProps(newProps){
		if (this.props.name !== newProps.name && !newProps.icons[newProps.name]){
			this.props.uiActions.getIcon(newProps.name);
		}
	}

	render(){
		var className = 'icon';
		if (this.props.className){
			className += ' '+this.props.className;
		}

		var svg = '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" x="0px" y="0px" viewBox="0 0 16 16" style="enable-background:new 0 0 16 16;" xml:space="preserve"></svg>';

		if (this.props.icons[this.props.name]){
			svg = this.props.icons[this.props.name];
		}

		return <span className={className} dangerouslySetInnerHTML={{ __html: svg}}></span>;
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		icons: (state.ui.icons ? state.ui.icons : {})
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Icon)
