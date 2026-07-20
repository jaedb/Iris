import React, { useEffect } from 'react';
import { useSelector, useDispatch } from 'react-redux';
import { useParams, useNavigate } from 'react-router-dom';
import Loader from '../components/Loader';
import { loadUri } from '../services/core/actions';
import { setWindowTitle } from '../services/ui/actions';
import { makeLoadingSelector, makeItemSelector } from '../util/selectors';
import { decodeUri } from '../util/format';
import { uriType } from '../util/helpers';

const UriRedirect = () => {
  const navigate = useNavigate();
  const dispatch = useDispatch();
  const { uri: rawUri } = useParams();
  const uri = decodeUri(rawUri);
  const loading = useSelector(makeLoadingSelector([`(.*)${uri}(.*)`]));
  const item = useSelector(makeItemSelector(uri));
  const redirect = () => navigate(`/${item.type || uriType(uri)}/${rawUri}`);

  useEffect(() => {
    if (item) {
      redirect();
    } else {
      dispatch(loadUri(uri));
    }

    dispatch(setWindowTitle(uri));
  }, []);

  useEffect(() => {
    if (item) {
      redirect();
    } else if (!loading) {
      dispatch(loadUri(uri));
      dispatch(setWindowTitle(uri));
    }
  }, [rawUri, loading]);


  return <Loader loading={loading} body />;
}

export default UriRedirect;
