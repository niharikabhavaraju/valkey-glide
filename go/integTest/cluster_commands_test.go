// Copyright Valkey GLIDE Project Contributors - SPDX Identifier: Apache-2.0

package integTest

import (
	"strings"

	"github.com/stretchr/testify/assert"
	"github.com/valkey-io/valkey-glide/go/glide/api"
	"github.com/valkey-io/valkey-glide/go/glide/api/config"
	"github.com/valkey-io/valkey-glide/go/glide/api/options"
)

func (suite *GlideTestSuite) TestClusterCustomCommandInfo() {
	client := suite.defaultClusterClient()
	result, err := client.CustomCommand([]string{"INFO"})

	assert.Nil(suite.T(), err)
	// INFO is routed to all primary nodes by default
	for _, value := range result.MultiValue() {
		assert.True(suite.T(), strings.Contains(value.(string), "# Stats"))
	}
}

func (suite *GlideTestSuite) TestClusterCustomCommandEcho() {
	client := suite.defaultClusterClient()
	result, err := client.CustomCommand([]string{"ECHO", "GO GLIDE GO"})

	assert.Nil(suite.T(), err)
	// ECHO is routed to a single random node
	assert.Equal(suite.T(), "GO GLIDE GO", result.SingleValue().(string))
}

func (suite *GlideTestSuite) TestInfoCluster() {
	DEFAULT_INFO_SECTIONS := []string{
		"Server",
		"Clients",
		"Memory",
		"Persistence",
		"Stats",
		"Replication",
		"CPU",
		"Modules",
		"Errorstats",
		"Cluster",
		"Keyspace",
	}

	client := suite.defaultClusterClient()
	t := suite.T()

	// info without options
	data, err := client.Info()
	assert.NoError(t, err)
	for _, info := range data {
		for _, section := range DEFAULT_INFO_SECTIONS {
			assert.Contains(t, info, "# "+section, "Section "+section+" is missing")
		}
	}

	// info with option or with multiple options without route
	sections := []api.Section{api.Cpu}
	if suite.serverVersion >= "7.0.0" {
		sections = append(sections, api.Memory)
	}
	opts := api.ClusterInfoOptions{
		InfoOptions: &api.InfoOptions{Sections: sections},
		Route:       nil,
	}
	response, err := client.InfoWithOptions(opts)
	assert.NoError(t, err)
	assert.True(t, response.IsMultiValue())
	for _, info := range response.MultiValue() {
		for _, section := range sections {
			assert.Contains(t, strings.ToLower(info), strings.ToLower("# "+string(section)), "Section "+section+" is missing")
		}
	}

	// same sections with random route
	opts = api.ClusterInfoOptions{
		InfoOptions: &api.InfoOptions{Sections: sections},
		Route:       config.RandomRoute.ToPtr(),
	}
	response, err = client.InfoWithOptions(opts)
	assert.NoError(t, err)
	assert.True(t, response.IsSingleValue())
	for _, section := range sections {
		assert.Contains(
			t,
			strings.ToLower(response.SingleValue()),
			strings.ToLower("# "+string(section)),
			"Section "+section+" is missing",
		)
	}

	// default sections, multi node route
	opts = api.ClusterInfoOptions{
		InfoOptions: nil,
		Route:       config.AllPrimaries.ToPtr(),
	}
	response, err = client.InfoWithOptions(opts)
	assert.NoError(t, err)
	assert.True(t, response.IsMultiValue())
	for _, info := range response.MultiValue() {
		for _, section := range DEFAULT_INFO_SECTIONS {
			assert.Contains(t, info, "# "+section, "Section "+section+" is missing")
		}
	}
}

func (suite *GlideTestSuite) TestTimeWithoutRoute() {
	client := suite.defaultClusterClient()
	options := options.RouteOption{Route: nil}
	result, err := client.TimeWithOptions(options)
	assert.NoError(suite.T(), err)
	assert.NotNil(suite.T(), result)
	assert.False(suite.T(), result.IsEmpty())
	assert.True(suite.T(), result.IsSingleValue())
	assert.NotEmpty(suite.T(), result.SingleValue())
	assert.IsType(suite.T(), "", result.SingleValue()[0])
	assert.Equal(suite.T(), 2, len(result.SingleValue()))
}

func (suite *GlideTestSuite) TestTimeWithAllNodesRoute() {
	client := suite.defaultClusterClient()
	route := config.Route(config.AllNodes)
	options := options.RouteOption{Route: route}
	result, err := client.TimeWithOptions(options)
	assert.NoError(suite.T(), err)
	assert.NotNil(suite.T(), result)
	assert.False(suite.T(), result.IsEmpty())
	assert.True(suite.T(), result.IsMultiValue())

	multiValue := result.MultiValue()
	assert.Greater(suite.T(), len(multiValue), 1)

	for nodeName, timeStrings := range multiValue {
		assert.NotEmpty(suite.T(), timeStrings, "Node %s should have time values", nodeName)
		for _, timeStr := range timeStrings {
			assert.IsType(suite.T(), "", timeStr)
		}
	}
}

func (suite *GlideTestSuite) TestTimeWithRandomRoute() {
	client := suite.defaultClusterClient()
	route := config.Route(config.RandomRoute)
	options := options.RouteOption{Route: route}
	result, err := client.TimeWithOptions(options)
	assert.NoError(suite.T(), err)
	assert.NotNil(suite.T(), result)
	assert.False(suite.T(), result.IsEmpty())
	assert.True(suite.T(), result.IsSingleValue())
	assert.NotEmpty(suite.T(), result.SingleValue())
	assert.IsType(suite.T(), "", result.SingleValue()[0])
	assert.Equal(suite.T(), 2, len(result.SingleValue()))
}

func (suite *GlideTestSuite) TestTimeWithInvalidRoute() {
	client := suite.defaultClusterClient()
	invalidRoute := config.Route(config.NewByAddressRoute("invalidHost", 9999))
	options := options.RouteOption{Route: invalidRoute}
	result, err := client.TimeWithOptions(options)
	assert.NotNil(suite.T(), err)
	assert.True(suite.T(), result.IsEmpty())
	assert.Empty(suite.T(), result.SingleValue())
}
