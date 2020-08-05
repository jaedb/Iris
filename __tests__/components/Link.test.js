import React from 'react';
import { BrowserRouter } from "react-router-dom";

// Testing-specific
import { shallow, mount, render } from 'enzyme';

// Test subjects
import Link from '../../src/js/components/Link';

describe('<Link />', () => {

	const dom = mount(
		<BrowserRouter>
			<Link to="test" className="test-classname">Link contents</Link>
		</BrowserRouter>
	);

	it('should render a valid <a> tag', () => {
		const a = dom.find('a');
		expect(a.length).toBe(1);
		expect(a.find('[href]').length).toBe(1);
		expect(a.text()).toEqual('Link contents');
	});

	it('should handle className prop', () => {
		expect(dom.find('a').hasClass('test-classname')).toBe(true);
	});
});