import React from 'react';
import { render } from '../test-wrapper';
import Link from '../../src/js/components/Link';

describe('<Link />', () => {

	const result = render(
		<Link to="test" className="test-classname">Link contents</Link>
	).toJSON();

	it('should render a valid <a> tag', () => {
		expect(result.type).toEqual('a');
		expect(result.props.href).toEqual('/test');
		expect(result.children.join('')).toEqual('Link contents');
		expect(result.props.className).toContain('test-classname');
	});
});