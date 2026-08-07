import { useParams } from 'react-router';
import { useSelector } from 'react-redux';

const useSearchQuery = () => {
  const {
    term = '',
    type = 'all',
    providers: rawProviders = 'all',
  } = useParams();
  const allProviders = useSelector(
    ({ mopidy: { uri_schemes } }) => uri_schemes || []
  ).map((str) => str.replace(/:/g,''));
  const providers = rawProviders == 'all'
                    ? [...allProviders]
                    : rawProviders.split(',').filter((str) => allProviders.indexOf(str) > -1);
  const providersString = providers.join(',');

  return {
    term,
    type,
    providers,
    allProviders,
    providersString,
  }
};

export default useSearchQuery;
