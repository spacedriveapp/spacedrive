import { locationName } from '../fixtures/location.json';
import { libraryName } from '../fixtures/onboarding.json';
import { locationRegex } from '../fixtures/routes';

describe('Location', () => {
	before(() => {
		cy.fastOnboarding(libraryName);
	});

	it('Add Location', () => {
		// Click on the "Add Location" button
		cy.get('button').contains('Add Location').click();

		// Get the input field for the new location inside modal
		cy.get('h2')
			.contains('New location')
			.parent()
			.find('input[name="path"]')
			.as('newLocationInput');

		// Get the form for the new location inside modal
		cy.get('@newLocationInput')
			.should('have.value', '')
			.then(($input) =>
				cy.window().then((win) => {
					const input = $input[0];
					if (input == null || !(input instanceof win.HTMLInputElement)) {
						throw new Error('Input not found');
					}

					return input.form;
				})
			)
			.as('newLocationForm');

		cy.get('@newLocationForm').within(() => {
			// Check if the "Open new location once added" checkbox is checked by default
			cy.get('label')
				.contains('Open new location once added')
				.parent()
				.find('input[type="checkbox"]')
				.should('be.checked');

			// Check if the "Add" button is disabled
			cy.get('button').contains('Add').as('addLocationButton');
			cy.get('@addLocationButton').should('be.disabled');

			// Check if the "Add" button is enabled after typing a valid location
			cy.get('@newLocationInput').type('/');
			cy.get('@addLocationButton').should('be.enabled');

			// Check if the "Add" button goes back to disabled after clearing the input
			cy.get('@newLocationInput').clear();
			cy.get('@addLocationButton').should('be.disabled');

			// Check if the "Add" is disabled and an error message is shown after typing an invalid location
			cy.get('@newLocationInput').type('nonExisting/path/I/hope');
			cy.get('@addLocationButton').should('be.disabled');
			cy.get('div > p').contains("location not found <path='nonExisting/path/I/hope'>");

			// Get location and add it as a new location
			cy.task<string>('repoRoot').then((repoRoot) => {
				cy.get('@newLocationInput').clear().type(`${repoRoot}/${locationName}`);
				cy.get('@addLocationButton').click();
			});
		});

		// Check if location was added to sidebar
		cy.get('div.group').children('div:contains("Locations") + a').contains(locationName);

		// Check if location is being scanned
		cy.get('button[id="job-manager-button"]').click();
		cy.get('span')
			.contains('Recent Jobs')
			.parent()
			.parent()
			.within(() =>
				cy
					.get('p')
					.invoke('text')
					.should('match', new RegExp(`^(Adding|Added) location "${locationName}"$`))
					.should('exist')
			);

		// Check redirect to location root page
		cy.url().should('match', locationRegex);
	});
});
