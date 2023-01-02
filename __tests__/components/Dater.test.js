import React from 'react';
import { render } from '../test-wrapper';
import Dater from '../../src/js/components/Dater';

jest.mock('react-redux', () => ({
  useSelector: jest.fn(),
}));

describe('<Dater />', () => {
	it('should render null when given invalid props', () => {

		// Invalid type
		const dom_1 = render(<Dater type="invalid-type" data={100000} />);
		expect(dom_1.toJSON()).toEqual(null);

		// Missing type
		const dom_2 = render(<Dater data={100000} />);
		expect(dom_2.toJSON()).toEqual(null);

		// Missing data
		const dom_4 = render(<Dater type="ago" />);
		expect(dom_4.toJSON()).toEqual(null);
	});

	it('should handle milliseconds', () => {
		const dom = render(<Dater type="length" data={30000} />);
		expect(dom.toJSON()).toEqual('0:30');
	});

	it('should handle date (yyyy-mm-dd)', () => {
		const dom = render(<Dater type="date" data="2015-10-23" />);
		expect(dom.toJSON()).toEqual("23/10/2015");
	});

	it('should handle date (mm/dd/yyyy)', () => {
		const dom = render(<Dater type="date" data="10/23/2015" />);
		expect(dom.toJSON()).toEqual("23/10/2015");
	});

	it('should handle ago (days)', () => {
		var date = new Date();
		date.setDate( date.getDate() - 3 );

		const dom = render(<Dater type="ago" data={date} />);
		expect(dom.toJSON().join('')).toEqual('3 days');
	});

	it('should handle ago (hours)', () => {
		var date = new Date();
		date.setTime( date.getTime() - 3 * 3600000 );

		const dom = render(<Dater type="ago" data={date} />);
		expect(dom.toJSON().join('')).toEqual('3 hours');
	});

	it('should handle ago (minutes)', () => {
		var date = new Date();
		date.setTime( date.getTime() - 3 * 60000 );

		const dom = render(<Dater type="ago" data={date} />);
		expect(dom.toJSON().join('')).toEqual('3 minutes');
	});

	it('should handle total time', () => {
		const data = [
			{
				duration: 5 * 60000
			},
			{
				duration: 5 * 60000
			},
			{
				duration: 5 * 60000
			}
		];
		const dom = render(<Dater type="total-time" data={data} />);
		expect(dom.toJSON().join('')).toEqual('15 mins');
	});
});