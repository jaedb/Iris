import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Loader from '../components/Loader';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import { makeLoadingSelector, makeItemSelector } from '../util/selectors';
import { decodeUri } from '../util/format';
import { uriType } from '../util/helpers';

class UriRedirect extends React.Component {
  componentDidMount() {
    const {
      uri,
      item,
      coreActions: {
        loadUri,
      },
    } = this.props;

    if (item) {
      this.redirect();
    } else {
      loadUri(uri);
    }

    this.setWindowTitle();
  }

  componentDidUpdate = ({
    uri: prevUri,
  }) => {
    const {
      uri,
      item,
      coreActions: {
        loadUri,
      },
    } = this.props;

    if (prevUri !== uri) {
      loadUri(uri);
      this.setWindowTitle();
    }

    if (item) {
      this.redirect();
    }
  }

  setWindowTitle = () => {
    const {
      uri,
      uiActions: {
        setWindowTitle,
      },
    } = this.props;

    setWindowTitle(uri);
  }

  redirect = () => {
    const {
      uri,
      item,
      history,
    } = this.props;

    history.replace(`/${item.type || uriType(uri)}/${uri}`);
  }

  render = () => <Loader body loading />;
}

const mapStateToProps = (state, ownProps) => {
  const { match: { params: { uri: rawUri } } } = ownProps;
  const uri = decodeUri(rawUri);
  const loadingSelector = makeLoadingSelector([`(.*)${uri}(.*)`]);
  const itemSelector = makeItemSelector(uri);

  return {
    uri,
    loading: loadingSelector(state),
    item: itemSelector(state),
  };
};

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  coreActions: bindActionCreators(coreActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(UriRedirect);
