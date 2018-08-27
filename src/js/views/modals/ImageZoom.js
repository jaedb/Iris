
import React from 'react';
import { connect } from 'react-redux';
import { createStore, bindActionCreators } from 'redux';

import Modal from './Modal';
import Thumbnail from '../../components/Thumbnail';

import * as helpers from '../../helpers';
import * as uiActions from '../../services/ui/actions';

class ImageZoom extends React.Component{

	constructor(props){
		super(props)
	}

	componentDidMount(){
		this.props.uiActions.setWindowTitle("Zoomed image");
	}

	render(){
		return (
			<Modal className="modal--image-zoom">			
				<img src={this.props.location.query.url} />
			</Modal>
		)
	}
}

const mapStateToProps = (state) => {
	return {}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(ImageZoom)